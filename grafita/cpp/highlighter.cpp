#include "highlighter.h"

#include <QtQml/QQmlEngine>
#include "syntax.cxx.h"

namespace {

// Qt formats a block in UTF-16 units; the lexer answers in UTF-8 byte offsets.
// Converting through the prefix is exact and cheap at a line's length, and
// keeps the two sides free to disagree about how they count.
int utf16Offset(const QByteArray &utf8, quint32 byteOffset)
{
    const int clamped = qMin<int>(static_cast<int>(byteOffset), utf8.size());
    return QString::fromUtf8(utf8.constData(), clamped).size();
}

} // namespace

GrafitaHighlighter::GrafitaHighlighter(QObject *parent)
    : QSyntaxHighlighter(parent)
{
}

void GrafitaHighlighter::setTarget(QQuickTextDocument *target)
{
    if (m_target == target)
        return;
    m_target = target;
    // Attaching re-highlights the whole document once; after that Qt only
    // re-runs the blocks that actually changed, which is what makes this cheap
    // while typing.
    setDocument(target ? target->textDocument() : nullptr);
    Q_EMIT targetChanged();
}

void GrafitaHighlighter::setLanguage(int language)
{
    if (m_language == language)
        return;
    m_language = language;
    Q_EMIT languageChanged();
    rehighlight();
}

void GrafitaHighlighter::setCommentColor(const QColor &colour)
{
    m_comment.setForeground(colour);
    Q_EMIT paletteChanged();
    rehighlight();
}

void GrafitaHighlighter::setStringColor(const QColor &colour)
{
    m_string.setForeground(colour);
    Q_EMIT paletteChanged();
    rehighlight();
}

void GrafitaHighlighter::setNumberColor(const QColor &colour)
{
    m_number.setForeground(colour);
    Q_EMIT paletteChanged();
    rehighlight();
}

void GrafitaHighlighter::setKeywordColor(const QColor &colour)
{
    m_keyword.setForeground(colour);
    Q_EMIT paletteChanged();
    rehighlight();
}

void GrafitaHighlighter::highlightBlock(const QString &text)
{
    if (m_language == 0)
        return;

    // Qt reports -1 for "no previous block"; the lexer's plain state is 0.
    const int incoming = previousBlockState() < 0 ? 0 : previousBlockState();
    const QByteArray utf8 = text.toUtf8();
    const rust::Str line(utf8.constData(), static_cast<size_t>(utf8.size()));

    const Coloured coloured =
        grafita_colour_line(line, static_cast<quint8>(m_language), static_cast<quint8>(incoming));

    for (const Run &run : coloured.runs) {
        const int start = utf16Offset(utf8, run.start);
        const int end = utf16Offset(utf8, run.end);
        if (end <= start)
            continue;
        switch (run.token) {
        case 0:
            setFormat(start, end - start, m_comment);
            break;
        case 1:
            setFormat(start, end - start, m_string);
            break;
        case 2:
            setFormat(start, end - start, m_number);
            break;
        case 3:
            setFormat(start, end - start, m_keyword);
            break;
        default:
            break;
        }
    }

    // What this line leaves for the next one. Qt uses it to decide which blocks
    // must be re-run when an edit changes a line's outgoing state — an edit that
    // opens a block comment re-colours what follows, and nothing else does.
    setCurrentBlockState(static_cast<int>(coloured.state));
}

void register_grafita_highlighter()
{
    qmlRegisterType<GrafitaHighlighter>("org.celestina.grafita.internal", 1, 0,
                                        "SyntaxHighlighter");
}
