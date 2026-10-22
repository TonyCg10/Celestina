//! What reaches the event loop from the shell's workers.
//!
//! Every provider runs on its own thread and publishes typed values through
//! the loop's `celestina_wayland::Sender<Message>`; the loop is the only place
//! that touches a surface (design §5.1). A variant arrives with the piece that
//! owns its type: the command verbs came with the command surface
//! (`command.rs`) and the tray's news with the tray host (CEL-1-G);
//! notifications come with CEL-1-F and the desktop index with CEL-1-E. The
//! launcher's close and a banner's request for the Notifications page are the
//! messages the app sends itself (see [`Message::LauncherClosed`] and
//! [`Message::OpenNotificationsPage`]).

use celestina_core::desktop_entry::DesktopEntry;
use celestina_wayland::{OutputId, SurfaceId};

use crate::command::Verb;
use crate::launcher::Switch;
use crate::providers::niri::{NiriModel, NiriRequest};
use crate::providers::notifications::NotificationsEvent;

/// One value a worker hands to the event loop.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// The compositor's reduced state, sent whole whenever it changed (at most
    /// once per coalescing window), and empty when the connection is lost.
    Niri(NiriModel),
    /// The suite's appearance file changed: reduced motion and text scale.
    Appearance(celestina_settings::Appearance),
    /// A compositor request was refused because the request queue was full or
    /// its worker had stopped; the loop shows the failure instead of queueing.
    NiriRequestRefused(NiriRequest),
    /// A verb of `org.celestina.Shell1`, from the bus worker.
    Command(Verb),
    /// The session's applications the launcher offers, from the desktop
    /// index's worker, whenever they changed.
    DesktopIndex(Vec<DesktopEntry>),
    /// The launcher on `surface` of `output` closed during an input call,
    /// which cannot destroy a surface; the app posts this to itself and
    /// destroys it here, then runs the `switch` that was chosen, if any
    /// (opening an Island page needs the Island's surface, which an input
    /// call cannot reach either).
    LauncherClosed {
        surface: SurfaceId,
        output: OutputId,
        switch: Option<Switch>,
    },
    /// Who serves notifications, and the notification server's state.
    Notifications(NotificationsEvent),
    /// A banner on this output was clicked without a default action: its
    /// Island opens the Notifications page. Sent by the loop to itself.
    OpenNotificationsPage(OutputId),
    /// The tray host's news: the watcher's owner, the items, an item's menu.
    Tray(crate::tray::TrayEvent),
    /// The wallpaper worker's news: a decoded picture, a fallback, a failure,
    /// an output another client holds, the folder's names.
    Wallpaper(crate::providers::wallpaper::WallpaperEvent),
    /// `shell.json` changed on disk (another program, or the shell's own
    /// write coming back): the value now in it, read off the loop's thread.
    /// Boxed: the whole settings, bar layout included, would size every
    /// message.
    ShellSettings(Box<crate::providers::settings::ShellRead>),
    /// News from the energy workers: idle, caffeine, night light, session.
    Energy(crate::energy::EnergyEvent),
    /// The system monitor's reading, or `None` once when its counters can no
    /// longer be read.
    Sysmon(Option<crate::providers::sysmon::SysReading>),
    /// The weather worker's reading, or `None` once when it is withdrawn (no
    /// location, or the last reading stopped being fresh).
    Weather(Option<crate::providers::weather::WeatherReading>),
    /// News from the clipboard worker: the latest rows (previews only), the
    /// pause state, a refusal. Tagged with the worker's generation.
    Clipboard(crate::clipboard::ClipboardMessage),
    /// News from the connectivity workers: network, Bluetooth, power, and
    /// what became of a request.
    Connectivity(crate::connectivity::ConnectivityEvent),
    /// A default audio device's level, from the audio worker; `None` is no
    /// reading, never 0 %.
    Audio(crate::providers::audio::AudioEvent),
    /// A monitor's brightness, from the brightness workers; `None` is
    /// unknown, never 0 %.
    Brightness(crate::providers::brightness::BrightnessEvent),
    /// The slider popup opened or closed: the display shows nothing of the
    /// kinds it drives while it is open, nor the levels it asked for that
    /// arrive after it closed. Posted by the loop to itself, because an
    /// input call controls only its own surface.
    OsdSuppressed(crate::osd::Suppression),
    /// One of the on-screen display's timers came due (posted by its
    /// calloop timer, which has no `RuntimeCtl`).
    Osd(crate::osd::OsdTick),
    /// Melibea's news (the minimised windows), stamped with the run of the
    /// worker that sent it. Boxed like the settings: it would size every
    /// message.
    Melibea(Box<crate::bubbles::BubblesNews>),
    /// Magnetita's news (the phone), stamped with the run of the worker that
    /// sent it.
    Phone(Box<crate::phone::PhoneNews>),
}
