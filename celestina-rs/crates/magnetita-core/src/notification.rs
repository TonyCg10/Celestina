//! A phone notification as the desktop mirrors it: raise, replace by id,
//! withdraw. The shape only; the wire is `magnetita-proto`'s.

/// A phone notification, or its withdrawal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    /// Stable per-notification id — the key for replace and cancel.
    pub id: String,
    /// The app that raised it, e.g. "WhatsApp". Empty on a cancel.
    pub app_name: String,
    pub title: String,
    pub text: String,
    /// True when the phone is *withdrawing* this notification, not raising it.
    pub is_cancel: bool,
}
