#include "highlighter.h"

#include <algorithm>
#include <iterator>
#include <limits>

#include <QtCore/QCoreApplication>
#include <QtCore/QMimeDatabase>
#include <QtCore/QMimeType>
#include <QtGui/QGuiApplication>
#include <QtGui/QTextBlock>
#include <QtGui/QTextCursor>
#include <QtGui/QTextBlockUserData>
#include <QtGui/QTextCharFormat>
#include <QtGui/QTextDocument>
#include <QtGui/QTextLayout>
#include <QtQml/QQmlEngine>

#include <KSyntaxHighlighting/Format>
#include <KSyntaxHighlighting/Repository>
#include <KSyntaxHighlighting/State>
#include <KSyntaxHighlighting/Theme>

#include "syntax.cxx.h"

namespace {

using KSyntaxHighlighting::Theme;
using Role = GrafitaHighlighter::Role;

// The one repository of the process. Loading it reads several hundred
// definitions' headers, so it is built once, on the first document, and never
// torn down: definitions handed out keep pointing into it until exit.
KSyntaxHighlighting::Repository &repository()
{
    static auto *const shared = new KSyntaxHighlighting::Repository;
    return *shared;
}

// What each of KSyntaxHighlighting's text styles is painted as. Normal text
// keeps the widget's own colour, so a definition never repaints prose.
Role roleFor(Theme::TextStyle style)
{
    switch (style) {
    case Theme::Keyword:
    case Theme::ControlFlow:
        return Role::Keyword;
    case Theme::Function:
    case Theme::Others:
        return Role::Function;
    case Theme::Variable:
    case Theme::DataType:
    case Theme::Information:
        return Role::Type;
    case Theme::BuiltIn:
    case Theme::Extension:
        return Role::BuiltIn;
    case Theme::Attribute:
    case Theme::Preprocessor:
    case Theme::Import:
        return Role::Attribute;
    case Theme::Char:
    case Theme::String:
    case Theme::VerbatimString:
    case Theme::SpecialString:
        return Role::String;
    case Theme::SpecialChar:
    case Theme::Annotation:
    case Theme::CommentVar:
        return Role::Escape;
    case Theme::DecVal:
    case Theme::BaseN:
    case Theme::Float:
    case Theme::Constant:
        return Role::Number;
    case Theme::Comment:
    case Theme::Documentation:
    case Theme::RegionMarker:
        return Role::Comment;
    case Theme::Operator:
        return Role::Operator;
    case Theme::Warning:
        return Role::Warning;
    case Theme::Alert:
    case Theme::Error:
        return Role::Error;
    case Theme::Normal:
        break;
    }
    return Role::Plain;
}

// Styles whose brackets are text rather than code, for the pair scan.
bool isQuoted(Theme::TextStyle style)
{
    switch (style) {
    case Theme::Char:
    case Theme::String:
    case Theme::VerbatimString:
    case Theme::SpecialString:
    case Theme::SpecialChar:
    case Theme::Comment:
    case Theme::Documentation:
    case Theme::Annotation:
    case Theme::CommentVar:
    case Theme::RegionMarker:
        return true;
    default:
        return false;
    }
}

// Marks a run painted from a quoted style, so the pair scan can tell without
// asking the engine again.
constexpr int QuotedProperty = QTextFormat::UserProperty + 1;

// A MIME type says nothing about colour when it is the catch-all for text or
// for bytes.
bool tellsALanguage(const QString &mime)
{
    return mime != QLatin1String("text/plain")
        && mime != QLatin1String("application/octet-stream");
}

// How much of the first line a content guess looks at. A shebang or an XML
// prolog fits many times over; a minified first line does not get copied.
constexpr qsizetype FirstLineBound = 512;

// The pair scan over a QTextDocument. Reads one block at a time and keeps it,
// because the scan walks offsets in order and almost every step stays in the
// block it is in.
class DocumentSource
{
public:
    explicit DocumentSource(const QTextDocument &document)
        : m_document(document)
        // QTextDocument counts a final paragraph separator no editor shows.
        , m_size(qMax(0, document.characterCount() - 1))
    {
    }

    int size() const { return m_size; }

    QChar at(int offset) const
    {
        loadText(offset);
        const int inBlock = offset - m_block.position();
        // The block's separator is not in its text: a line break.
        return inBlock < m_text.size() ? m_text.at(inBlock) : QChar(u'\n');
    }

    bool quoted(int offset) const
    {
        loadText(offset);
        loadQuoted();
        const int inBlock = offset - m_block.position();
        // The ranges never overlap and are sorted when they are loaded, so a
        // minified line with a hundred thousand of them is still a binary
        // search per step.
        const auto after = std::upper_bound(
            m_quoted.cbegin(), m_quoted.cend(), inBlock,
            [](int at, const std::pair<int, int> &range) { return at < range.first; });
        return after != m_quoted.cbegin() && inBlock < std::prev(after)->second;
    }

private:
    void loadText(int offset) const
    {
        if (m_block.isValid() && m_block.contains(offset))
            return;
        m_block = m_document.findBlock(offset);
        m_text = m_block.text();
        m_quoted.clear();
        m_quotedLoaded = false;
    }

    // Only asked for once a bracket has been found, so a caret beside no
    // bracket never reads a block's formats.
    void loadQuoted() const
    {
        if (m_quotedLoaded)
            return;
        m_quotedLoaded = true;
        if (const QTextLayout *layout = m_block.layout()) {
            for (const QTextLayout::FormatRange &range : layout->formats()) {
                if (range.format.hasProperty(QuotedProperty))
                    m_quoted.append({range.start, range.start + range.length});
            }
            std::sort(m_quoted.begin(), m_quoted.end());
        }
    }

    const QTextDocument &m_document;
    const int m_size;
    mutable QTextBlock m_block;
    mutable QString m_text;
    mutable QList<std::pair<int, int>> m_quoted;
    mutable bool m_quotedLoaded = false;
};

// The pair scan over a string, for the tests: a non-space character in
// `quoted` marks the same offset as quoted.
class StringSource
{
public:
    StringSource(QString text, QString quoted)
        : m_text(std::move(text))
        , m_quoted(std::move(quoted))
    {
    }

    int size() const { return int(m_text.size()); }
    QChar at(int offset) const { return m_text.at(offset); }
    bool quoted(int offset) const
    {
        return offset < m_quoted.size() && m_quoted.at(offset) != u' ';
    }

private:
    QString m_text;
    QString m_quoted;
};

} // namespace

namespace grafita::syntax {

KSyntaxHighlighting::Definition definitionFor(const QString &fileName, QStringView firstLine)
{
    auto &known = repository();
    if (!fileName.isEmpty()) {
        const auto byName = known.definitionForFileName(fileName);
        if (byName.isValid())
            return byName;
    }
    if (fileName.isEmpty() && firstLine.isEmpty())
        return {};

    // Name and content together: the shared MIME database knows a shebang or
    // an XML prolog where no extension says anything.
    const QMimeDatabase mimes;
    const QMimeType mime = mimes.mimeTypeForFileNameAndData(
        fileName, firstLine.left(FirstLineBound).toUtf8());
    if (!mime.isValid() || mime.isDefault())
        return {};
    QStringList candidates{mime.name()};
    candidates += mime.aliases();
    candidates += mime.allAncestors();
    for (const QString &candidate : std::as_const(candidates)) {
        if (!tellsALanguage(candidate))
            continue;
        const auto byType = known.definitionForMimeType(candidate);
        if (byType.isValid())
            return byType;
    }
    return {};
}

} // namespace grafita::syntax

namespace {

// What the engine left at the end of a block, kept on the block so the next
// one starts from it.
class BlockState : public QTextBlockUserData
{
public:
    KSyntaxHighlighting::State state;
};

KSyntaxHighlighting::State stateAfter(const QTextBlock &block)
{
    if (!block.isValid())
        return {};
    const auto *kept = static_cast<const BlockState *>(block.userData());
    return kept ? kept->state : KSyntaxHighlighting::State();
}

bool isBracket(QChar character)
{
    switch (character.unicode()) {
    case u'(': case u')': case u'[': case u']': case u'{': case u'}':
        return true;
    default:
        return false;
    }
}

} // namespace

GrafitaHighlighter::GrafitaHighlighter(QObject *parent)
    : QSyntaxHighlighter(parent)
{
}

void GrafitaHighlighter::setDefinition(const KSyntaxHighlighting::Definition &definition)
{
    if (definition == this->definition())
        return;
    KSyntaxHighlighting::AbstractHighlighter::setDefinition(definition);
    rehighlight();
}

void GrafitaHighlighter::highlightBlock(const QString &text)
{
    const KSyntaxHighlighting::State incoming = stateAfter(currentBlock().previous());
    const KSyntaxHighlighting::State outgoing = highlightLine(text, incoming);

    auto *kept = static_cast<BlockState *>(currentBlockUserData());
    const bool changed = !kept || kept->state != outgoing;
    if (!kept) {
        kept = new BlockState;
        setCurrentBlockUserData(kept);
    }
    kept->state = outgoing;

    // Qt re-runs the next block, in this same pass, exactly when this block's
    // integer state changes. The engine's state is not an integer, so the
    // integer is a revision bumped whenever the engine's state moved: opening
    // a comment re-colours what follows in one edit, and an edit that leaves
    // the state alone stops at its own line. (KSyntaxHighlighting's own
    // SyntaxHighlighter queues one re-highlight per following block instead,
    // and each of those reached the widget as a separate text change.)
    const int revision = currentBlockState();
    if (changed)
        setCurrentBlockState(revision < 0 || revision == std::numeric_limits<int>::max()
                                 ? 0 : revision + 1);
    else
        setCurrentBlockState(revision);
}

void GrafitaHighlighter::setTarget(QQuickTextDocument *target)
{
    if (m_target == target)
        return;
    QObject::disconnect(m_targetDestroyed);
    QObject::disconnect(m_contentsChange);
    m_target = target;
    if (target) {
        // By the time `destroyed` is emitted the QPointer already reads null,
        // so a binding that re-reads `target` on this signal gets null rather
        // than a freed object. The QTextDocument itself is guarded by
        // QSyntaxHighlighter's own pointer.
        m_targetDestroyed = connect(target, &QObject::destroyed, this,
                                    [this]() { Q_EMIT targetChanged(); });
        m_contentsChange = connect(target->textDocument(), &QTextDocument::contentsChange,
                                   this, &GrafitaHighlighter::onContentsChange);
    }
    // The definition is chosen before attaching, without a pass of its own:
    // attaching re-highlights the whole document once, and after that Qt only
    // re-runs the blocks that actually changed, which is what makes this cheap
    // while typing.
    if (chooseDefinition(target ? target->textDocument() : nullptr))
        Q_EMIT definitionChanged();
    setDocument(target ? target->textDocument() : nullptr);
    Q_EMIT targetChanged();
}

void GrafitaHighlighter::setFileName(const QString &fileName)
{
    if (m_fileName == fileName)
        return;
    m_fileName = fileName;
    Q_EMIT fileNameChanged();
    // The session names the file before it hands over the new text, so the
    // pass is queued: the text that arrives in this same turn is coloured as
    // it is inserted, by the new definition, and the queued pass is then
    // skipped. A "save as" that only renames re-colours on the next turn.
    selectDefinition();
}

QString GrafitaHighlighter::definitionName() const
{
    const auto current = definition();
    return current.isValid() ? current.name() : QString();
}

bool GrafitaHighlighter::chooseDefinition(const QTextDocument *text)
{
    QString firstLine;
    if (text)
        firstLine = text->firstBlock().text().left(FirstLineBound);

    const auto chosen = grafita::syntax::definitionFor(m_fileName, firstLine);
    m_namedByFile = chosen.isValid()
        && repository().definitionForFileName(m_fileName) == chosen;
    if (chosen == definition())
        return false;
    KSyntaxHighlighting::AbstractHighlighter::setDefinition(chosen);
    return true;
}

void GrafitaHighlighter::selectDefinition()
{
    m_selectionQueued = false;
    if (!chooseDefinition(document()))
        return;
    Q_EMIT definitionChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::scheduleSelection()
{
    if (m_selectionQueued)
        return;
    m_selectionQueued = true;
    QMetaObject::invokeMethod(this, &GrafitaHighlighter::selectDefinition,
                              Qt::QueuedConnection);
}

void GrafitaHighlighter::onContentsChange(int position, int /*removed*/, int added)
{
    const QTextDocument *text = document();
    // A change that replaced the whole text was coloured as it went in, with
    // whatever definition and palette are current: a queued pass would repeat
    // it.
    if (text && position == 0 && added >= text->characterCount() - 1)
        m_recolouredWhole = true;
    // Only an edit that reaches the first line can change a content guess,
    // and a definition the name settled is not guessed at all.
    if (m_namedByFile)
        return;
    if (!text || position > text->firstBlock().length())
        return;
    scheduleSelection();
}

QList<int> GrafitaHighlighter::matchBracket(int position) const
{
    const QTextDocument *text = document();
    if (!text)
        return {};
    // Most caret moves are beside no bracket: answered without reading a
    // block's text or formats.
    if (!isBracket(text->characterAt(position)) && !isBracket(text->characterAt(position - 1)))
        return {};
    const DocumentSource source(*text);
    const auto [bracket, partner] = grafita::syntax::matchBracket(source, position);
    if (bracket < 0 || partner < 0)
        return {};
    return {bracket, partner};
}

void GrafitaHighlighter::applyFormat(int offset, int length,
                                     const KSyntaxHighlighting::Format &format)
{
    // An invalid format is a line coloured by no definition: plain text.
    if (length <= 0 || !format.isValid())
        return;

    const Theme::TextStyle style = format.textStyle();
    const Role role = roleFor(style);
    QTextCharFormat painted;
    if (role != Role::Plain)
        painted.setForeground(m_colours[std::size_t(role)]);
    // Comments lean, the way every editor sets them apart from code.
    if (role == Role::Comment)
        painted.setFontItalic(true);
    // A definition's own emphasis is kept — Markdown's bold and italic are
    // the text's meaning, not decoration — but never its own colours, which
    // would bypass the suite's palette. With no theme set, `isBold` and the
    // rest answer the definition's override alone.
    const Theme none = theme();
    if (format.hasBoldOverride() && format.isBold(none))
        painted.setFontWeight(QFont::Bold);
    if (format.hasItalicOverride())
        painted.setFontItalic(format.isItalic(none));
    if (format.hasUnderlineOverride() && format.isUnderline(none))
        painted.setFontUnderline(true);
    if (format.hasStrikeThroughOverride() && format.isStrikeThrough(none))
        painted.setFontStrikeOut(true);
    if (isQuoted(style))
        painted.setProperty(QuotedProperty, true);

    if (painted.properties().isEmpty())
        return;
    setFormat(offset, length, painted);
}

void GrafitaHighlighter::setColour(Role role, const QColor &colour)
{
    auto &slot = m_colours[std::size_t(role)];
    if (slot == colour)
        return;
    slot = colour;
    Q_EMIT paletteChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::scheduleRehighlight()
{
    // Every request clears the mark, queued pass or not: a change asked for
    // after the whole text was replaced is not in that text's colours yet.
    m_recolouredWhole = false;
    if (m_rehighlightQueued)
        return;
    m_rehighlightQueued = true;
    // Queued on this object, so it runs on this object's thread and never after
    // the object is gone: a pending queued call dies with its receiver.
    QMetaObject::invokeMethod(this, &GrafitaHighlighter::flushRehighlight,
                              Qt::QueuedConnection);
}

void GrafitaHighlighter::flushRehighlight()
{
    m_rehighlightQueued = false;
    // Without a document there is nothing to colour; attaching one later
    // colours it then.
    if (document() && !m_recolouredWhole)
        rehighlight();
}

void register_grafita_highlighter()
{
    qmlRegisterType<GrafitaHighlighter>("org.celestina.grafita.internal", 1, 0,
                                        "SyntaxHighlighter");
}

namespace {

// One offscreen application for the test process. Qt takes the first thread
// that builds a QObject as its main thread, and only that thread's posted
// events and timers run, so every hook that builds one starts here and the
// Rust tests that call them share one test function.
void ensureTestApplication()
{
    if (QCoreApplication::instance())
        return;
    qputenv("QT_QPA_PLATFORM", "offscreen");
    static int argc = 1;
    static char name[] = "grafita-test";
    static char *argv[] = {name, nullptr};
    new QGuiApplication(argc, argv);
}

} // namespace

rust::String grafita_definition_name(rust::Str fileName, rust::Str firstLine)
{
    ensureTestApplication();
    const auto name = QString::fromUtf8(fileName.data(), qsizetype(fileName.size()));
    const auto line = QString::fromUtf8(firstLine.data(), qsizetype(firstLine.size()));
    const auto chosen = grafita::syntax::definitionFor(name, line);
    const QByteArray utf8 = chosen.isValid() ? chosen.name().toUtf8() : QByteArray();
    return rust::String(utf8.constData(), std::size_t(utf8.size()));
}

BracketPair grafita_bracket_pair(rust::Str text, rust::Str quoted, int position)
{
    const StringSource source(QString::fromUtf8(text.data(), qsizetype(text.size())),
                              QString::fromUtf8(quoted.data(), qsizetype(quoted.size())));
    const auto [bracket, partner] = grafita::syntax::matchBracket(source, position);
    return BracketPair{bracket, partner};
}

namespace {

// Whether the first character of the document's last line of code is painted
// as quoted.
bool lastLineQuoted(const QTextDocument &document)
{
    const QTextBlock last = document.lastBlock().previous();
    if (const QTextLayout *layout = last.layout()) {
        for (const QTextLayout::FormatRange &range : layout->formats()) {
            if (range.start == 0 && range.format.hasProperty(QuotedProperty))
                return true;
        }
    }
    return false;
}

} // namespace

Recolouring grafita_recolour_after_comment(std::uint32_t lines)
{
    ensureTestApplication();

    QString text;
    for (std::uint32_t line = 0; line < lines; ++line)
        text += QStringLiteral("int x%1 = %1; // c\n").arg(line);
    QTextDocument document(text);
    // A document reports its edits only once it has a layout, as the
    // widget's always has.
    document.documentLayout();
    GrafitaHighlighter highlighter;
    highlighter.setFileName(QStringLiteral("programa.c"));
    highlighter.setDocument(&document);
    for (int turn = 0; turn < 10; ++turn)
        QCoreApplication::processEvents();

    // The last line's trailing `// c` is a comment before the edit too, so it
    // is its code — `int x… = …;` — that is asked about.
    const bool before = lastLineQuoted(document);
    std::uint32_t edits = 0;
    QObject::connect(&document, &QTextDocument::contentsChange, &document,
                     [&edits](int, int, int) { ++edits; });
    QTextCursor(&document).insertText(QStringLiteral("/*"));
    // Long enough for a queued re-highlight per line to have shown up.
    for (std::uint32_t turn = 0; turn < 4 * lines; ++turn)
        QCoreApplication::processEvents();

    return Recolouring{edits, before, lastLineQuoted(document)};
}
