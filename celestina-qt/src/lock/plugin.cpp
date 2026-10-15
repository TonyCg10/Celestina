// The plugin Qt's Wayland platform loads to decide what role this process's
// windows get. It answers "lock surface" for all of them, which is why this is
// its own process: the shell's windows are layer surfaces, one integration is
// chosen per process, and the two cannot both be right in one program.

#include <QtWaylandClient/private/qwaylandshellintegrationplugin_p.h>

#include "locksurface.h"

class SessionLockIntegrationPlugin final
    : public QtWaylandClient::QWaylandShellIntegrationPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QWaylandShellIntegrationFactoryInterface_iid
                      FILE "celestina-lock.json")

public:
    QtWaylandClient::QWaylandShellIntegration *create(
        const QString &key,
        const QStringList &parameters
    ) override
    {
        Q_UNUSED(parameters)
        if (key.compare(QLatin1String("celestina-lock"), Qt::CaseInsensitive)
            != 0) {
            return nullptr;
        }
        return new SessionLockIntegration();
    }
};

#include "plugin.moc"
