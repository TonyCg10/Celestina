// Syntax colouring for Grafita's editing surface.
//
// Hand-written C++ for one concrete reason: colouring a Qt text document
// without rewriting its text means overriding QSyntaxHighlighter::highlightBlock,
// and CXX-Qt 0.9 cannot subclass a Qt class or override its virtuals from Rust.
// Every alternative reachable from Rust alone would have to replace the widget's
// text with markup, which would break the reconciliation that keeps a CRLF file
// from being rewritten.
//
// No colouring rule lives here. This asks the Rust lexer what the runs are and
// paints them; what counts as a comment, a string or a keyword is decided in
// grafita-core and nowhere else.
#pragma once

#include <QtCore/QMetaObject>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtGui/QSyntaxHighlighter>
#include <QtGui/QTextCharFormat>
// Included rather than forward-declared: moc needs the complete type to build
// a pointer meta-type for the `target` property.
#include <QtQuick/QQuickTextDocument>

class GrafitaHighlighter : public QSyntaxHighlighter
{
    Q_OBJECT
    // The TextEdit's document, handed over from QML as `body.textDocument`.
    // The TextEdit owns it, so this only watches it: when it goes first the
    // property reads null and `targetChanged` says so.
    Q_PROPERTY(QQuickTextDocument *target READ target WRITE setTarget NOTIFY targetChanged)
    // The numeric language from the Rust side. 0 is plain text, which colours
    // nothing — the default, so an unset or unknown language is never an error.
    Q_PROPERTY(int language READ language WRITE setLanguage NOTIFY languageChanged)
    // Colours, injected from CelestinaTheme so the palette stays in one place
    // and this file hardcodes none of it. Setting all four, as QML does at
    // start-up, re-colours the document once, not four times.
    Q_PROPERTY(QColor commentColor READ commentColor WRITE setCommentColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor stringColor READ stringColor WRITE setStringColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor numberColor READ numberColor WRITE setNumberColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor keywordColor READ keywordColor WRITE setKeywordColor NOTIFY paletteChanged)

public:
    explicit GrafitaHighlighter(QObject *parent = nullptr);

    QQuickTextDocument *target() const { return m_target.data(); }
    void setTarget(QQuickTextDocument *target);

    int language() const { return m_language; }
    void setLanguage(int language);

    QColor commentColor() const { return m_comment.foreground().color(); }
    void setCommentColor(const QColor &colour);
    QColor stringColor() const { return m_string.foreground().color(); }
    void setStringColor(const QColor &colour);
    QColor numberColor() const { return m_number.foreground().color(); }
    void setNumberColor(const QColor &colour);
    QColor keywordColor() const { return m_keyword.foreground().color(); }
    void setKeywordColor(const QColor &colour);

Q_SIGNALS:
    void targetChanged();
    void languageChanged();
    void paletteChanged();

protected:
    void highlightBlock(const QString &text) override;

private:
    // Coalesces the palette setters that arrive in one turn of the event loop
    // into a single re-colouring pass.
    void scheduleRehighlight();
    void flushRehighlight();

    QPointer<QQuickTextDocument> m_target;
    QMetaObject::Connection m_targetDestroyed;
    bool m_rehighlightQueued = false;
    int m_language = 0;
    QTextCharFormat m_comment;
    QTextCharFormat m_string;
    QTextCharFormat m_number;
    QTextCharFormat m_keyword;
};

void register_grafita_highlighter();
