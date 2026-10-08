// Syntax colouring and bracket matching for Grafita's editing surface.
//
// Hand-written C++ for one concrete reason: colouring a Qt text document
// without rewriting its text means applying formats from a QSyntaxHighlighter
// subclass, and CXX-Qt 0.9 cannot subclass a Qt class or override its virtuals
// from Rust. Every alternative reachable from Rust alone would have to replace
// the widget's text with markup, which would break the reconciliation that
// keeps a CRLF file from being rewritten.
//
// What counts as a keyword, a string or a comment in each of several hundred
// languages is KDE's KSyntaxHighlighting (the `syntax-highlighting` package),
// the engine Kate and KWrite colour with. This file only chooses the definition
// and decides what each of its text styles looks like, from colours QML injects
// from CelestinaTheme.
#pragma once

#include <array>
#include <cstdint>
#include <utility>

#include <QtCore/QList>
#include <QtCore/QMetaObject>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QString>
#include <QtCore/QStringView>
#include <QtGui/QColor>
// Included rather than forward-declared: moc needs the complete type to build
// a pointer meta-type for the `target` property.
#include <QtQuick/QQuickTextDocument>

#include <KSyntaxHighlighting/Definition>
#include <QtGui/QSyntaxHighlighter>

#include <KSyntaxHighlighting/AbstractHighlighter>

#include "rust/cxx.h"

struct BracketPair;
struct Recolouring;

// A QSyntaxHighlighter driven by KSyntaxHighlighting's AbstractHighlighter —
// the engine's documented pattern for an output of one's own — rather than its
// ready-made SyntaxHighlighter, which re-colours the lines after a state change
// one queued edit at a time.
class GrafitaHighlighter : public QSyntaxHighlighter, public KSyntaxHighlighting::AbstractHighlighter
{
    Q_OBJECT
    // The TextEdit's document, handed over from QML as `body.textDocument`.
    // The TextEdit owns it, so this only watches it: when it goes first the
    // property reads null and `targetChanged` says so.
    Q_PROPERTY(QQuickTextDocument *target READ target WRITE setTarget NOTIFY targetChanged)
    // The name of the file the document is coloured by. Empty, or a name no
    // definition claims, falls back to what the first line looks like and then
    // to plain text, so an unknown file is never an error.
    Q_PROPERTY(QString fileName READ fileName WRITE setFileName NOTIFY fileNameChanged)
    // The chosen definition's name, empty for plain text.
    Q_PROPERTY(QString definitionName READ definitionName NOTIFY definitionChanged)
    // Colours, injected from CelestinaTheme so the palette stays in one place
    // and this file hardcodes none of it. Setting them all, as QML does at
    // start-up or when the scheme changes, re-colours the document once.
    Q_PROPERTY(QColor keywordColor READ keywordColor WRITE setKeywordColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor functionColor READ functionColor WRITE setFunctionColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor typeColor READ typeColor WRITE setTypeColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor builtInColor READ builtInColor WRITE setBuiltInColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor attributeColor READ attributeColor WRITE setAttributeColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor stringColor READ stringColor WRITE setStringColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor escapeColor READ escapeColor WRITE setEscapeColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor numberColor READ numberColor WRITE setNumberColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor commentColor READ commentColor WRITE setCommentColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor operatorColor READ operatorColor WRITE setOperatorColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor warningColor READ warningColor WRITE setWarningColor NOTIFY paletteChanged)
    Q_PROPERTY(QColor errorColor READ errorColor WRITE setErrorColor NOTIFY paletteChanged)

public:
    // What a text style is painted as. Several of KSyntaxHighlighting's
    // thirty-one styles share one: the palette has a handful of hues, not one
    // per style.
    enum class Role {
        Plain,
        Keyword,
        Function,
        Type,
        BuiltIn,
        Attribute,
        String,
        Escape,
        Number,
        Comment,
        Operator,
        Warning,
        Error,
        Count
    };

    explicit GrafitaHighlighter(QObject *parent = nullptr);

    QQuickTextDocument *target() const { return m_target.data(); }
    void setTarget(QQuickTextDocument *target);

    QString fileName() const { return m_fileName; }
    void setFileName(const QString &fileName);

    QString definitionName() const;
    // Re-colours the whole document with the new definition.
    void setDefinition(const KSyntaxHighlighting::Definition &definition) override;

    // The bracket beside `position` and its partner, as UTF-16 offsets into
    // the document, or an empty list when the caret is beside no bracket or
    // its partner is not within reach. Reads the document; never changes it.
    Q_INVOKABLE QList<int> matchBracket(int position) const;

#define GRAFITA_COLOUR(getter, setter, role)                                    \
    QColor getter() const { return m_colours[std::size_t(Role::role)]; }      \
    void setter(const QColor &colour) { setColour(Role::role, colour); }
    GRAFITA_COLOUR(keywordColor, setKeywordColor, Keyword)
    GRAFITA_COLOUR(functionColor, setFunctionColor, Function)
    GRAFITA_COLOUR(typeColor, setTypeColor, Type)
    GRAFITA_COLOUR(builtInColor, setBuiltInColor, BuiltIn)
    GRAFITA_COLOUR(attributeColor, setAttributeColor, Attribute)
    GRAFITA_COLOUR(stringColor, setStringColor, String)
    GRAFITA_COLOUR(escapeColor, setEscapeColor, Escape)
    GRAFITA_COLOUR(numberColor, setNumberColor, Number)
    GRAFITA_COLOUR(commentColor, setCommentColor, Comment)
    GRAFITA_COLOUR(operatorColor, setOperatorColor, Operator)
    GRAFITA_COLOUR(warningColor, setWarningColor, Warning)
    GRAFITA_COLOUR(errorColor, setErrorColor, Error)
#undef GRAFITA_COLOUR

Q_SIGNALS:
    void targetChanged();
    void fileNameChanged();
    void definitionChanged();
    void paletteChanged();

protected:
    void highlightBlock(const QString &text) override;
    // Where KSyntaxHighlighting's theme would be consulted. Grafita has no
    // theme file: the role table and the injected colours are the theme.
    void applyFormat(int offset, int length, const KSyntaxHighlighting::Format &format) override;

private:
    void setColour(Role role, const QColor &colour);
    // Coalesces the palette setters that arrive in one turn of the event loop
    // into a single re-colouring pass.
    void scheduleRehighlight();
    void flushRehighlight();
    // Picks the definition again: when the name changes, when a document is
    // attached, and — while the name settled nothing — when the first line
    // changes.
    void selectDefinition();
    // Picks the definition for this name and `text`'s first line without
    // re-colouring anything; true when it changed.
    bool chooseDefinition(const QTextDocument *text);
    void scheduleSelection();
    void onContentsChange(int position, int removed, int added);

    QPointer<QQuickTextDocument> m_target;
    QMetaObject::Connection m_targetDestroyed;
    QMetaObject::Connection m_contentsChange;
    QString m_fileName;
    // True when the definition came from the file's name, so an edit to the
    // first line cannot change it.
    bool m_namedByFile = false;
    bool m_rehighlightQueued = false;
    bool m_selectionQueued = false;
    // Set when the whole text was replaced since the last queued pass was
    // asked for, which then has nothing left to do.
    bool m_recolouredWhole = false;
    std::array<QColor, std::size_t(Role::Count)> m_colours;
};

namespace grafita::syntax {

// The definition a file is coloured by: by its name, then by its name and first
// line together (a shebang, an XML prolog), else an invalid one, which is plain
// text.
KSyntaxHighlighting::Definition definitionFor(const QString &fileName, QStringView firstLine);

// How far a partner is looked for, in characters either way from the bracket.
// Past it a bracket simply pairs with nothing; the caret never waits on a
// minified megabyte.
inline constexpr int bracketScanLimit = 100000;

// The pair scan over any text. `Source` answers `size()`, `at(offset)` and
// `quoted(offset)` — the last is whether the offset is inside a string or a
// comment. The bracket after the caret is tried before the one before it, and
// the first that has a partner wins. A bracket outside a string or a comment
// skips brackets inside one; a bracket inside one counts everything.
template<typename Source>
std::pair<int, int> matchBracket(const Source &source, int position, int limit = bracketScanLimit)
{
    for (const int candidate : {position, position - 1}) {
        if (candidate < 0 || candidate >= source.size())
            continue;
        const QChar bracket = source.at(candidate);
        QChar partner;
        int step = 0;
        switch (bracket.unicode()) {
        case u'(': partner = u')'; step = 1; break;
        case u'[': partner = u']'; step = 1; break;
        case u'{': partner = u'}'; step = 1; break;
        case u')': partner = u'('; step = -1; break;
        case u']': partner = u'['; step = -1; break;
        case u'}': partner = u'{'; step = -1; break;
        default: continue;
        }
        const bool inside = source.quoted(candidate);
        int depth = 0;
        int walked = 0;
        for (int at = candidate + step; at >= 0 && at < source.size() && walked < limit;
             at += step, ++walked) {
            if (!inside && source.quoted(at))
                continue;
            const QChar here = source.at(at);
            if (here == bracket) {
                ++depth;
            } else if (here == partner) {
                if (depth == 0)
                    return {candidate, at};
                --depth;
            }
        }
    }
    return {-1, -1};
}

} // namespace grafita::syntax

void register_grafita_highlighter();

// Test hooks over the same functions the highlighter uses; see src/syntax.rs.
rust::String grafita_definition_name(rust::Str fileName, rust::Str firstLine);
BracketPair grafita_bracket_pair(rust::Str text, rust::Str quoted, int position);
Recolouring grafita_recolour_after_comment(std::uint32_t lines);
