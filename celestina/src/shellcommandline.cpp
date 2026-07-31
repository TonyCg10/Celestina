#include "shellcommandline.h"

namespace {
// A keybind line is short. These bounds keep a mistyped or hostile invocation
// from reaching the bus as a large message.
constexpr qsizetype maxOptionCount = 16;
constexpr qsizetype maxNameLength = 32;
constexpr qsizetype maxValueLength = 256;

bool isVerbName(const QString &name)
{
    if (name.isEmpty() || name.size() > maxNameLength)
        return false;
    if (!name.at(0).isLower())
        return false;

    for (const QChar character : name) {
        const bool allowed = (character >= u'a' && character <= u'z')
            || (character >= u'0' && character <= u'9') || character == u'-';
        if (!allowed)
            return false;
    }
    return true;
}

bool isOptionName(const QString &name)
{
    if (name.isEmpty() || name.size() > maxNameLength)
        return false;
    if (!name.at(0).isLower())
        return false;

    for (const QChar character : name) {
        if (!character.isLetterOrNumber() && character != u'-')
            return false;
    }
    return true;
}

// Keybinds and scripts write plain text. Numbers and booleans are recognized
// so an option keeps the type its consumer expects; everything else stays a
// string rather than being guessed at.
QVariant optionValue(const QString &text)
{
    if (text == QStringLiteral("true"))
        return true;
    if (text == QStringLiteral("false"))
        return false;

    bool numeric = false;
    const qlonglong number = text.toLongLong(&numeric);
    if (numeric)
        return number;

    return text;
}
} // namespace

ShellCommandLine parseShellCommandLine(const QStringList &arguments)
{
    ShellCommandLine line;
    if (arguments.isEmpty()) {
        line.error = QStringLiteral("usage: celestina msg <verb> [key=value ...]");
        return line;
    }

    line.verb = arguments.first();
    if (!isVerbName(line.verb)) {
        line.error = QStringLiteral("'%1' is not a verb name").arg(
            line.verb.left(maxNameLength)
        );
        return line;
    }
    line.readsState = line.verb == QStringLiteral("get-state");

    const QStringList options = arguments.mid(1);
    if (options.size() > maxOptionCount) {
        line.error = QStringLiteral("a verb takes at most %1 options")
                         .arg(maxOptionCount);
        return line;
    }

    for (const QString &option : options) {
        const qsizetype separator = option.indexOf(u'=');
        if (separator < 0) {
            line.error = QStringLiteral("'%1' is not a key=value option")
                             .arg(option.left(maxNameLength));
            return line;
        }

        const QString name = option.left(separator);
        const QString value = option.mid(separator + 1);
        if (!isOptionName(name)) {
            line.error = QStringLiteral("'%1' is not an option name")
                             .arg(name.left(maxNameLength));
            return line;
        }
        if (value.size() > maxValueLength) {
            line.error = QStringLiteral("the value of '%1' is too long").arg(name);
            return line;
        }
        if (line.options.contains(name)) {
            line.error = QStringLiteral("'%1' is given twice").arg(name);
            return line;
        }

        line.options.insert(name, optionValue(value));
    }

    return line;
}
