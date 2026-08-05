#include "siderita/windowparent.h"

#include <QtQml/qqml.h>

#ifdef SIDERITA_HAS_XDG_FOREIGN
#include <QtGui/QGuiApplication>
#include <QtGui/QWindow>
#include <QtGui/qguiapplication_platform.h>
#include <QtGui/qpa/qplatformwindow_p.h>

#include <wayland-client.h>

#include "xdg-foreign-unstable-v2-client-protocol.h"

namespace {

// The prefix the portal specification puts in front of a Wayland handle. An
// X11 parent arrives as `x11:<xid>` and is not this backend's business.
constexpr QLatin1StringView kWaylandPrefix("wayland:");

struct ImporterHunt {
    zxdg_importer_v2 *importer = nullptr;
};

void registryGlobal(void *data, wl_registry *registry, uint32_t name,
                    const char *interface, uint32_t version)
{
    Q_UNUSED(version);
    auto *hunt = static_cast<ImporterHunt *>(data);
    if (qstrcmp(interface, zxdg_importer_v2_interface.name) == 0) {
        hunt->importer = static_cast<zxdg_importer_v2 *>(
                wl_registry_bind(registry, name, &zxdg_importer_v2_interface, 1));
    }
}

void registryGlobalRemove(void *data, wl_registry *registry, uint32_t name)
{
    Q_UNUSED(data);
    Q_UNUSED(registry);
    Q_UNUSED(name);
}

const wl_registry_listener kRegistryListener = {
    registryGlobal,
    registryGlobalRemove,
};

} // namespace
#endif

SideritaWindowParent::SideritaWindowParent(QObject *parent)
    : QObject(parent)
{
}

SideritaWindowParent::~SideritaWindowParent()
{
#ifdef SIDERITA_HAS_XDG_FOREIGN
    if (m_imported != nullptr) {
        zxdg_imported_v2_destroy(m_imported);
    }
    if (m_importer != nullptr) {
        zxdg_importer_v2_destroy(m_importer);
    }
#endif
}

bool SideritaWindowParent::adopt(QWindow *window, const QString &parentWindow)
{
#ifdef SIDERITA_HAS_XDG_FOREIGN
    if (window == nullptr || !parentWindow.startsWith(kWaylandPrefix)) {
        return false;
    }
    if (m_imported != nullptr) {
        // Already a child. Importing twice would leave the first relationship
        // behind with nothing to end it.
        return true;
    }

    auto *application = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();
    if (application == nullptr) {
        return false; // Not a Wayland session: offscreen, X11, a test run.
    }

    // A window has no surface until it is created, and this is called as the
    // dialog opens.
    window->create();
    auto *native =
            window->nativeInterface<QNativeInterface::Private::QWaylandWindow>();
    if (native == nullptr || native->surface() == nullptr) {
        return false;
    }

    wl_display *display = application->display();
    if (m_importer == nullptr) {
        ImporterHunt hunt;
        wl_registry *registry = wl_display_get_registry(display);
        wl_registry_add_listener(registry, &kRegistryListener, &hunt);
        // One roundtrip on the connection Qt already owns, from Qt's own
        // thread: the globals arrive, and nothing else is waiting on them.
        wl_display_roundtrip(display);
        wl_registry_destroy(registry);
        m_importer = hunt.importer;
    }
    if (m_importer == nullptr) {
        return false; // A compositor without xdg-foreign; nothing to do.
    }

    const QByteArray handle = parentWindow.mid(kWaylandPrefix.size()).toUtf8();
    m_imported = zxdg_importer_v2_import_toplevel(m_importer, handle.constData());
    if (m_imported == nullptr) {
        return false;
    }
    zxdg_imported_v2_set_parent_of(m_imported, native->surface());
    wl_display_flush(display);
    return true;
#else
    Q_UNUSED(window);
    Q_UNUSED(parentWindow);
    return false;
#endif
}

void register_siderita_window_parent()
{
    // The same separate namespace the native list model uses: cxx-qt registers
    // org.celestina.siderita declaratively, and Qt forbids also registering a
    // type into it imperatively.
    qmlRegisterType<SideritaWindowParent>("org.celestina.siderita.internal", 1, 0,
                                          "SideritaWindowParent");
}
