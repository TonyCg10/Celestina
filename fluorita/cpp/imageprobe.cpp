#include "fluorita/imageprobe.h"

#include <fcntl.h>
#include <unistd.h>

#include <QtCore/QByteArray>
#include <QtCore/QFile>
#include <QtGui/QImageReader>

namespace {

// A read-only descriptor opened from raw path bytes, closed when this goes out
// of scope unless a QFile has taken over the closing.
//
// `open` on the bytes is the one call that names a file exactly, whatever its
// name spells.
class ReadDescriptor
{
public:
    explicit ReadDescriptor(const QByteArray &pathBytes)
        : m_descriptor(::open(pathBytes.constData(), O_RDONLY | O_CLOEXEC))
    {
    }

    ReadDescriptor(const ReadDescriptor &) = delete;
    ReadDescriptor &operator=(const ReadDescriptor &) = delete;
    ReadDescriptor(ReadDescriptor &&) = delete;
    ReadDescriptor &operator=(ReadDescriptor &&) = delete;

    ~ReadDescriptor()
    {
        if (m_descriptor >= 0) {
            ::close(m_descriptor);
        }
    }

    // Hands the descriptor to `file`, which closes it from here on. Returns
    // false — and keeps ownership — when the QFile refuses it.
    bool adoptInto(QFile &file)
    {
        if (m_descriptor < 0 ||
            !file.open(m_descriptor, QIODevice::ReadOnly, QFileDevice::AutoCloseHandle)) {
            return false;
        }
        m_descriptor = -1;
        return true;
    }

private:
    int m_descriptor;
};

} // namespace

QSize fluorita_probe_image(const QString &key)
{
    // Byte-level decoding: QUrl::fromPercentEncoding would answer a QString and
    // lose exactly the names this indirection exists for.
    const QByteArray pathBytes = QByteArray::fromPercentEncoding(key.toLatin1());
    // A published key is absolute; a relative one would resolve against this
    // process's working directory and measure some other file.
    if (pathBytes.isEmpty() || !pathBytes.startsWith('/')) {
        return QSize();
    }

    ReadDescriptor descriptor(pathBytes);
    QFile file;
    if (!descriptor.adoptInto(file)) {
        return QSize();
    }

    QImageReader reader(&file);
    // EXIF orientation swaps width and height for a quarter turn, and the
    // budget must be judged on what would actually be allocated.
    reader.setAutoTransform(true);
    if (!reader.canRead()) {
        return QSize();
    }
    const QSize size = reader.size();
    // Some formats do not answer before decoding; an unknown size is reported
    // as empty so the caller refuses rather than guesses a budget.
    return size.isValid() ? size : QSize();
}
