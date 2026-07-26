//! The `kdeconnect.sftp` plugin — how the phone hands us the keys to its files.
//!
//! To browse the phone we ask it to start an SFTP server ([`request_packet`], a
//! `kdeconnect.sftp.request` carrying `{"startBrowsing": true}`), and it replies
//! with a `kdeconnect.sftp` packet describing where to connect: a port, a
//! one-session `user`/`password`, the root `path`, and — optionally — the
//! storage volumes as parallel `multiPaths`/`pathNames` arrays (internal
//! storage, SD card, …) so each can be a labelled entry point.
//!
//! The reply carries **no host**: the SFTP server runs on the phone we are
//! already linked to, so the address is the connection's, supplied by the
//! transport — this pure module only decodes what is in the packet. A reply may
//! instead be an `errorMessage` (the phone refused or failed), which is a real
//! answer, not a parse failure, so it has its own [`SftpReply`] arm.
//!
//! Pure: this turns packets into a typed [`SftpMount`]; opening the sshfs mount
//! from it is the daemon's I/O, tested there.

use serde_json::{json, Value};

use crate::packet::NetworkPacket;

/// The reply packet type: the phone's SFTP mount details.
pub const TYPE_SFTP: &str = "kdeconnect.sftp";

/// The request packet type: asks the phone to start its SFTP server.
pub const TYPE_SFTP_REQUEST: &str = "kdeconnect.sftp.request";

/// The request that makes the phone start serving SFTP and reply with a
/// [`TYPE_SFTP`] packet.
pub fn request_packet(id: i64) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_SFTP_REQUEST, json!({ "startBrowsing": true }))
}

/// What a `kdeconnect.sftp` reply says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SftpReply {
    /// The phone is serving SFTP with these details.
    Mount(SftpMount),
    /// The phone refused or could not serve (e.g. storage permission denied).
    Error(String),
}

/// The details for opening the phone's SFTP mount. The host is *not* here — it is
/// the linked phone's address, which the transport already knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SftpMount {
    /// Some phones echo their `ip`; when absent the caller uses the link address.
    pub ip: Option<String>,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// The SFTP root to mount.
    pub path: String,
    /// Absolute paths of the storage volumes to surface, paired with
    /// [`path_names`](SftpMount::path_names). Empty if the phone sent none.
    pub multi_paths: Vec<String>,
    /// Display names parallel to [`multi_paths`](SftpMount::multi_paths).
    pub path_names: Vec<String>,
}

impl SftpMount {
    /// The volumes to surface as `(remote path, label)`. Uses the phone's
    /// `multiPaths`/`pathNames` when they line up; falls back to the single root
    /// `path` labelled "All files" — the same fallback the reference client uses
    /// when a phone sends no volume list.
    pub fn directories(&self) -> Vec<(String, String)> {
        if !self.multi_paths.is_empty() && self.multi_paths.len() == self.path_names.len() {
            self.multi_paths
                .iter()
                .cloned()
                .zip(self.path_names.iter().cloned())
                .collect()
        } else if !self.multi_paths.is_empty() {
            // Paths without matching labels: name each by its own path.
            self.multi_paths
                .iter()
                .map(|p| (p.clone(), p.clone()))
                .collect()
        } else {
            vec![(self.path.clone(), "All files".to_owned())]
        }
    }
}

/// Reads a `kdeconnect.sftp` reply, or `None` for a different type or a body
/// missing a required field. An `errorMessage` body is a valid [`SftpReply::Error`].
pub fn read_sftp(packet: &NetworkPacket) -> Option<SftpReply> {
    if !packet.is(TYPE_SFTP) {
        return None;
    }
    let body = packet.body.as_object()?;

    if let Some(message) = body.get("errorMessage").and_then(Value::as_str) {
        return Some(SftpReply::Error(message.to_owned()));
    }

    Some(SftpReply::Mount(SftpMount {
        ip: body.get("ip").and_then(Value::as_str).map(str::to_owned),
        port: read_port(body.get("port")?)?,
        user: body.get("user")?.as_str()?.to_owned(),
        password: body.get("password")?.as_str()?.to_owned(),
        path: body.get("path")?.as_str()?.to_owned(),
        multi_paths: read_str_array(body.get("multiPaths")),
        path_names: read_str_array(body.get("pathNames")),
    }))
}

/// The port arrives as a JSON number from most phones and a string from some;
/// accept either, rejecting anything outside a valid port.
fn read_port(value: &Value) -> Option<u16> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|p| u16::try_from(p).ok()),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// A JSON string array to a `Vec<String>`; anything else (including absent) is
/// empty.
fn read_str_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{read_sftp, request_packet, SftpReply, TYPE_SFTP_REQUEST};
    use crate::packet::NetworkPacket;

    #[test]
    fn the_request_asks_the_phone_to_start_browsing() {
        let packet = request_packet(1);
        assert!(packet.is(TYPE_SFTP_REQUEST));
        assert_eq!(packet.body["startBrowsing"], true);
    }

    #[test]
    fn a_full_reply_parses_every_field_and_pairs_volumes() {
        let raw = r#"{"id":1,"type":"kdeconnect.sftp","body":{
            "ip":"10.0.0.85","port":1739,"user":"kdeconnect","password":"s3cr3t",
            "path":"/storage/emulated/0",
            "multiPaths":["/storage/emulated/0","/storage/1A2B-3C4D"],
            "pathNames":["Internal storage","SD card"]}}"#;
        let SftpReply::Mount(m) = read_sftp(&NetworkPacket::parse(raw).unwrap()).unwrap() else {
            panic!("expected a mount");
        };
        assert_eq!(m.ip.as_deref(), Some("10.0.0.85"));
        assert_eq!(m.port, 1739);
        assert_eq!(m.user, "kdeconnect");
        assert_eq!(m.password, "s3cr3t");
        assert_eq!(
            m.directories(),
            vec![
                ("/storage/emulated/0".to_owned(), "Internal storage".to_owned()),
                ("/storage/1A2B-3C4D".to_owned(), "SD card".to_owned()),
            ]
        );
    }

    #[test]
    fn a_port_sent_as_a_string_is_accepted() {
        let raw = r#"{"id":1,"type":"kdeconnect.sftp","body":{
            "port":"1739","user":"kdeconnect","password":"p","path":"/"}}"#;
        let SftpReply::Mount(m) = read_sftp(&NetworkPacket::parse(raw).unwrap()).unwrap() else {
            panic!("expected a mount");
        };
        assert_eq!(m.port, 1739);
    }

    #[test]
    fn no_volume_list_falls_back_to_all_files_at_the_root() {
        let raw = r#"{"id":1,"type":"kdeconnect.sftp","body":{
            "port":1739,"user":"kdeconnect","password":"p","path":"/storage/emulated/0"}}"#;
        let SftpReply::Mount(m) = read_sftp(&NetworkPacket::parse(raw).unwrap()).unwrap() else {
            panic!("expected a mount");
        };
        assert_eq!(
            m.directories(),
            vec![("/storage/emulated/0".to_owned(), "All files".to_owned())]
        );
    }

    #[test]
    fn an_error_reply_is_its_own_answer() {
        let raw = r#"{"id":1,"type":"kdeconnect.sftp","body":{
            "errorMessage":"Permission denied"}}"#;
        assert_eq!(
            read_sftp(&NetworkPacket::parse(raw).unwrap()),
            Some(SftpReply::Error("Permission denied".to_owned()))
        );
    }

    #[test]
    fn a_reply_missing_a_required_field_does_not_parse() {
        // No password.
        let raw = r#"{"id":1,"type":"kdeconnect.sftp","body":{
            "port":1739,"user":"kdeconnect","path":"/"}}"#;
        assert!(read_sftp(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn a_non_sftp_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_sftp(&ping).is_none());
    }
}
