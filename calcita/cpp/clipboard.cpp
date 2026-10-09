#include "calcita/clipboard.h"

#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>

void calcita_set_clipboard_text(const QString& text)
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
        return;
    }
    clipboard->setText(text, QClipboard::Clipboard);
}
