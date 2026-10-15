// celestina-lock — the program that covers this session and will not uncover
// it without PAM.
//
// It is a separate process from the shell for two reasons, one mechanical and
// one that matters more. Qt chooses one Wayland shell integration per process
// and the shell's surfaces are layer surfaces, so a lock surface cannot share
// that process. And a lock that dies with the bar, or takes the bar with it,
// is a lock nobody should trust: here, the shell crashing leaves the session
// locked, and this crashing leaves it locked too, because
// `ext-session-lock-v1` puts that guarantee in the compositor.
//
// The lifecycle is deliberately small:
//
//   lock the session  ->  cover every output  ->  wait
//   authenticated     ->  unlock and exit 0
//   anything else     ->  stay locked
//
// It reports one line on stdout when the compositor has confirmed the lock, so
// the caller can sequence a suspend behind it rather than guessing. There is
// no line, and no exit code, that means "unlocked" without an authentication.

#include <cstdio>

#include <QCommandLineOption>
#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QCommandLineParser>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlComponent>
#include <QQmlContext>
#include <QScreen>
#include <QWindow>
#include <QTextStream>
#include <QTimer>
#include <QUrl>

#include <utility>

#include "lockauthenticator.h"
#include "lockbackdrop.h"
#include "locksession.h"
#include "lockuncover.h"

namespace {

// What the caller reads on stdout. One word, so a shell script can wait for it
// as easily as the shell can.
constexpr const char *confirmedLine = "locked";

// How long the session stays covered after PAM has said yes, so the backdrop
// can travel back to the geometry the session's own wallpaper is already in.
//
// It must be at least as long as `LockScreen.qml`'s retreat, which is one
// `motionFast` pause followed by `motionSlow` of travel. The margin is for the
// frame the compositor still has to present. Being generous here costs a person
// a few tenths of a second; being short costs them only a visible seam, and
// neither can leave the session unlocked early or locked forever.
constexpr int uncoverCeilingMs = 550;

enum ExitCode {
    Unlocked = 0,
    // The session could not be locked at all. The caller must not suspend on
    // this, and must not assume the screen is covered.
    NotLocked = 2,
    // The compositor ended the lock without us asking. The session's state is
    // the compositor's, not ours.
    Finished = 3,
};

} // namespace

int main(int argc, char **argv)
{
    // Before QGuiApplication: the platform plugin reads both of these while it
    // starts, and the whole point of this process is which shell integration
    // it picks. The path is where the build put ours; an installed lock finds
    // it beside itself. Appending rather than replacing leaves Qt's own
    // plugins reachable.
    const QByteArray pluginPath = qgetenv("CELESTINA_LOCK_PLUGINS").isEmpty()
        ? QByteArray(CELESTINA_LOCK_PLUGIN_PATH)
        : qgetenv("CELESTINA_LOCK_PLUGINS");
    QByteArray searchPath = qgetenv("QT_PLUGIN_PATH");
    if (!searchPath.isEmpty())
        searchPath.append(':');
    searchPath.append(pluginPath);
    qputenv("QT_PLUGIN_PATH", searchPath);
    // An explicit setting wins, which is what let the EGL hang below be
    // isolated: the same binary was run against Qt's own xdg-shell to prove
    // the integration was not the cause.
    if (qgetenv("QT_WAYLAND_SHELL_INTEGRATION").isEmpty())
        qputenv("QT_WAYLAND_SHELL_INTEGRATION", "celestina-lock");

    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("celestina-lock"));
    // By API as well as by environment: the factory that looks for shell
    // integrations reads the application's library paths, and those are
    // computed once.
    QCoreApplication::addLibraryPath(QString::fromLocal8Bit(pluginPath));

    QCommandLineParser parser;
    parser.setApplicationDescription(
        QStringLiteral("Covers this session until PAM says otherwise."));
    parser.addHelpOption();
    QCommandLineOption serviceOption(
        QStringLiteral("service"),
        QStringLiteral("PAM service whose stack decides."),
        QStringLiteral("name"), QStringLiteral("login"));
    parser.addOption(serviceOption);
    parser.process(app);

    auto *authenticator = new LockAuthenticator(&app);
    authenticator->setService(parser.value(serviceOption));

    QQmlApplicationEngine engine;

    // The shell's own trick, for the same reason: `CelestinaStyle` is a module
    // on disk whose URI must be a directory name, so it is reached through a
    // runtime symlink and its parent added as an import path. Without it the
    // lock screen cannot import the material the rest of the shell is made of.
    {
        QString runtime =
            QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
        if (runtime.isEmpty())
            runtime = QDir::tempPath();
        const QString importRoot =
            QDir(runtime).filePath(QStringLiteral("celestina-shell-import"));
        QDir().mkpath(importRoot);
        const QString styleLink =
            importRoot + QStringLiteral("/CelestinaStyle");
        QFile::remove(styleLink);
        QFile::link(qEnvironmentVariable("CELESTINA_STYLE_PATH",
                                         QStringLiteral(CELESTINA_STYLE_DIR)),
                    styleLink);
        engine.addImportPath(importRoot);
    }
    engine.rootContext()->setContextProperty(QStringLiteral("lockAuthenticator"),
                                             authenticator);

    QQmlComponent component(&engine);
    component.loadFromModule("CelestinaLock", "LockScreen");
    if (!component.isReady()) {
        return NotLocked;
    }

    // One cover per output, and an output that appears while the session is
    // locked gets one immediately. ADR 0004: an output that cannot be covered
    // keeps the session locked rather than exposing itself, so a screen whose
    // window fails to build is left uncovered by the *compositor*, which shows
    // its own blank — it is never a reason to unlock.
    // What the shell says this session is showing. It may never arrive, and
    // every cover below is built without waiting for it.
    auto *backdrop = new LockBackdrop(&app);

    // A cover paints the file chosen for its own output, or nothing — in which
    // case the surface keeps the deliberate canvas it has always shown. C++
    // owns the path-to-URL conversion so a space or a `#` in a filename stays
    // filename data rather than becoming URL syntax.
    const auto dress = [backdrop](QWindow *window, QScreen *screen) {
        if (!window || !screen)
            return;
        const QString source = backdrop->sourceFor(screen->name());
        window->setProperty("wallpaperSource", source);
        window->setProperty(
            "wallpaperUrl",
            source.isEmpty() ? QUrl() : QUrl::fromLocalFile(source));
    };

    QHash<QScreen *, QWindow *> covers;
    const auto cover = [&covers, &component, &engine, dress](QScreen *screen) {
        if (!screen || covers.contains(screen))
            return;
        auto *object = component.create(engine.rootContext());
        auto *window = qobject_cast<QWindow *>(object);
        if (!window) {
            qCritical() << "celestina-lock: the lock screen is not a window";
            delete object;
            return;
        }
        window->setScreen(screen);
        // Before `show`, so a cover that already knows its picture never maps
        // one frame of bare canvas first.
        dress(window, screen);
        // The compositor sends this surface its size; the geometry here is
        // only what Qt needs before that configure arrives.
        window->setGeometry(screen->geometry());
        // The configure is collected inside the lock surface's own
        // constructor, so showing here is already legal; nothing needs to be
        // pumped by hand.
        window->show();
        covers.insert(screen, window);
    };

    for (QScreen *const screen : QGuiApplication::screens())
        cover(screen);
    QObject::connect(&app, &QGuiApplication::screenAdded, &app, cover);
    // The ordinary case: every cover is already up and mapped when the shell's
    // line arrives, so they are dressed after the fact rather than before it.
    QObject::connect(backdrop, &LockBackdrop::changed, &app, [&covers, dress]() {
        for (auto entry = covers.constBegin(); entry != covers.constEnd();
             ++entry) {
            dress(entry.value(), entry.key());
        }
    });
    QObject::connect(&app, &QGuiApplication::screenRemoved, &app,
                     [&covers](QScreen *screen) {
                         if (QWindow *const window = covers.take(screen))
                             window->deleteLater();
                     });

    if (covers.isEmpty()) {
        // No output to cover means nothing was covered. Refusing is the only
        // honest answer; the caller must not sequence a suspend behind it.
        return NotLocked;
    }

    // Only now: Qt builds its shell integration when the first window needs a
    // role, so the lock does not exist until a cover has been created. Asking
    // before that reported "could not lock" for a lock that was about to be
    // granted.
    LockSession *const lock = LockSession::instance();
    if (!lock) {
        // The integration refused, which it does when the compositor has no
        // ext-session-lock-v1. Nothing is covered and nothing pretends to be.
        return NotLocked;
    }

    const auto announce = []() {
        // The compositor has the session. Only now may anything be sequenced
        // behind this lock.
        // Flushed at once: a caller sequencing a suspend behind this line
        // must not wait on a buffer.
        std::fputs(confirmedLine, stdout);
        std::fputc('\n', stdout);
        std::fflush(stdout);
    };
    QObject::connect(lock, &LockSession::confirmed, &app, announce);
    // The compositor confirms as soon as the first lock surface is bound, and
    // that happens while the covers above are being built — before there was
    // anything here to hear it. A lock already confirmed is announced now
    // rather than waited for forever.
    if (lock->isConfirmed())
        announce();

    QObject::connect(lock, &LockSession::finished, &app, []() {
        // Not ours to unlock. Leave without touching it.
        QGuiApplication::exit(Finished);
    });

    // The one place this program unlocks anything. Every other verdict — a
    // refusal, a verifier that would not run, a child that crashed — leaves
    // the lock exactly where it is and the person free to try again.
    auto *uncover = new LockUncover(uncoverCeilingMs, &app);

    // What the retreat is allowed to do: change what is on screen. Every cover
    // is told, none is asked, and none of them is on the path to the release
    // below.
    QObject::connect(uncover, &LockUncover::retreat, &app, [&covers]() {
        for (QWindow *const window : std::as_const(covers))
            window->setProperty("retreating", true);
    });

    QObject::connect(uncover, &LockUncover::uncover, &app, [lock]() {
        lock->release();
        QGuiApplication::exit(Unlocked);
    });

    QObject::connect(
        authenticator, &LockAuthenticator::answered, &app,
        [uncover](LockAuthenticator::Verdict verdict) {
            if (verdict != LockAuthenticator::Verdict::Authenticated)
                return;
            uncover->begin();
        });

    return app.exec();
}
