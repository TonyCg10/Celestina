#include "highlighter.h"

#include <limits>

#include <QtQml/QQmlEngine>
#include "syntax.cxx.h"

GrafitaHighlighter::GrafitaHighlighter(QObject *parent)
    : QSyntaxHighlighter(parent)
{
}

void GrafitaHighlighter::setTarget(QQuickTextDocument *target)
{
    if (m_target == target)
        return;
    QObject::disconnect(m_targetDestroyed);
    m_target = target;
    if (target) {
        // By the time `destroyed` is emitted the QPointer already reads null,
        // so a binding that re-reads `target` on this signal gets null rather
        // than a freed object. The QTextDocument itself is guarded by
        // QSyntaxHighlighter's own pointer.
        m_targetDestroyed = connect(target, &QObject::destroyed, this,
                                    [this]() { Q_EMIT targetChanged(); });
    }
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
    // At once rather than coalesced: a language arrives once per document,
    // and deferring it would leave the new text painted with the old
    // language's colours for a turn of the event loop.
    rehighlight();
}

void GrafitaHighlighter::setCommentColor(const QColor &colour)
{
    m_comment.setForeground(colour);
    Q_EMIT paletteChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::setStringColor(const QColor &colour)
{
    m_string.setForeground(colour);
    Q_EMIT paletteChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::setNumberColor(const QColor &colour)
{
    m_number.setForeground(colour);
    Q_EMIT paletteChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::setKeywordColor(const QColor &colour)
{
    m_keyword.setForeground(colour);
    Q_EMIT paletteChanged();
    scheduleRehighlight();
}

void GrafitaHighlighter::scheduleRehighlight()
{
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
    if (document())
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

    // The runs arrive in the UTF-16 units `setFormat` takes, so they are
    // painted as given; the offset rule lives in grafita-core alone.
    // `setFormat` itself ignores a run that starts past the block and trims
    // one that runs over its end.
    constexpr quint32 largest = static_cast<quint32>(std::numeric_limits<int>::max());
    for (const Run &run : coloured.runs) {
        if (run.length == 0 || run.start > largest)
            continue;
        const int start = static_cast<int>(run.start);
        const int length = static_cast<int>(qMin(run.length, largest));
        switch (run.token) {
        case 0:
            setFormat(start, length, m_comment);
            break;
        case 1:
            setFormat(start, length, m_string);
            break;
        case 2:
            setFormat(start, length, m_number);
            break;
        case 3:
            setFormat(start, length, m_keyword);
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
