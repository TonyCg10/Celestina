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

/// The details for opening the phone's SFTP mount. The host is *not* here — it
/// is the linked phone's address, which the transport already knows, and which
/// is the only address a mount may target. A body's `ip` field is read and
/// discarded: honouring it would let the peer redirect the mount, and the
/// one-session password with it, to a host of its choosing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SftpMount {
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

    let multi_paths = read_str_array(body.get("multiPaths"));
    if !multi_paths.iter().all(|path| is_valid_remote_path(path)) {
        return None;
    }
    Some(SftpReply::Mount(SftpMount {
        port: read_port(body.get("port")?)?,
        user: read_user(body.get("user")?)?,
        password: read_password(body.get("password")?)?,
        path: read_path(body.get("path")?)?,
        multi_paths,
        path_names: read_str_array(body.get("pathNames")),
    }))
}

/// Longest peer-supplied SFTP string accepted. A real account name and a real
/// Android storage path are far shorter; the bound only stops a peer from
/// handing the mount an argument no honest phone would send.
const MAX_SFTP_FIELD: usize = 4096;

/// Longest account name accepted, matching the length limits real systems put
/// on a user name.
const MAX_SFTP_USER: usize = 64;

/// The account name goes straight into `sshfs`'s `user@host:path` positional
/// argument, so it is held to a strict allowlist. Anything else — an empty
/// value, a separator, whitespace, or a leading `-` that would turn the
/// argument into an option `sshfs` forwards to `ssh` — is not a user name and
/// is refused before the daemon can spawn anything with it.
fn read_user(value: &Value) -> Option<String> {
    let user = value.as_str()?;
    let valid = !user.is_empty()
        && user.len() <= MAX_SFTP_USER
        && user
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !user.starts_with('-');
    valid.then(|| user.to_owned())
}

/// The remote root shares the same positional argument as the user name, so it
/// must be an ordinary absolute path: no leading `-`, no control byte that a
/// log or an option file would read as a new line, and bounded.
fn read_path(value: &Value) -> Option<String> {
    let path = value.as_str()?;
    is_valid_remote_path(path).then(|| path.to_owned())
}

fn is_valid_remote_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= MAX_SFTP_FIELD
        && !path.chars().any(char::is_control)
        && !path.contains('\0')
}

/// The one-session password is written to `sshfs`'s stdin as a single line, so
/// it may hold anything except the newline that would end that line early, a
/// NUL, or more bytes than a session credential ever needs.
fn read_password(value: &Value) -> Option<String> {
    let password = value.as_str()?;
    let valid = password.len() <= MAX_SFTP_FIELD
        && !password.contains(['\n', '\r', '\0'])
        && !password.is_empty();
    valid.then(|| password.to_owned())
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
        assert_eq!(m.port, 1739);
        assert_eq!(m.user, "kdeconnect");
        assert_eq!(m.password, "s3cr3t");
        assert_eq!(
            m.directories(),
            vec![
                (
                    "/storage/emulated/0".to_owned(),
                    "Internal storage".to_owned()
                ),
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

    /// A `kdeconnect.sftp` body with `user` and `path` chosen by the caller.
    fn mount_body(user: &str, path: &str) -> NetworkPacket {
        NetworkPacket::new(
            1,
            super::TYPE_SFTP,
            serde_json::json!({
                "port": 1739,
                "user": user,
                "password": "s3cr3t",
                "path": path,
            }),
        )
    }

    #[test]
    fn a_user_that_would_become_an_sshfs_option_is_refused() {
        // argv[1] is `<user>@<host>:<path>`; a user starting with `-` makes the
        // whole argument an option sshfs forwards to ssh, which runs
        // ProxyCommand through a shell.
        for hostile in [
            "-oProxyCommand=touch /tmp/pwned #",
            "-o",
            "kde connect",
            "kde@connect",
            "kde:connect",
            "kde/connect",
            "",
        ] {
            assert!(
                read_sftp(&mount_body(hostile, "/storage/emulated/0")).is_none(),
                "{hostile:?} must not decode into a mount"
            );
        }
    }

    #[test]
    fn a_user_longer_than_any_account_name_is_refused() {
        let long = "a".repeat(65);
        assert!(read_sftp(&mount_body(&long, "/storage/emulated/0")).is_none());
    }

    #[test]
    fn a_path_that_is_not_an_ordinary_absolute_path_is_refused() {
        for hostile in [
            "-oProxyCommand=touch /tmp/pwned",
            "storage/emulated/0",
            "",
            "/storage\nemulated",
            "/storage\remulated",
            "/storage\u{0}emulated",
        ] {
            assert!(
                read_sftp(&mount_body("kdeconnect", hostile)).is_none(),
                "{hostile:?} must not decode into a mount"
            );
        }
    }

    #[test]
    fn a_hostile_volume_path_refuses_the_whole_reply() {
        let packet = NetworkPacket::new(
            1,
            super::TYPE_SFTP,
            serde_json::json!({
                "port": 1739,
                "user": "kdeconnect",
                "password": "s3cr3t",
                "path": "/storage/emulated/0",
                "multiPaths": ["/storage/emulated/0", "-oProxyCommand=id"],
                "pathNames": ["Internal storage", "SD card"],
            }),
        );
        assert!(read_sftp(&packet).is_none());
    }

    #[test]
    fn a_password_that_could_end_its_stdin_line_early_is_refused() {
        for hostile in ["", "s3cr3t\nmore", "s3cr3t\rmore", "s3cr3t\u{0}"] {
            let packet = NetworkPacket::new(
                1,
                super::TYPE_SFTP,
                serde_json::json!({
                    "port": 1739,
                    "user": "kdeconnect",
                    "password": hostile,
                    "path": "/storage/emulated/0",
                }),
            );
            assert!(
                read_sftp(&packet).is_none(),
                "{hostile:?} must not decode into a mount"
            );
        }
    }

    #[test]
    fn an_ordinary_reply_still_decodes_after_the_checks() {
        let SftpReply::Mount(m) = read_sftp(&mount_body("kdeconnect", "/storage/emulated/0"))
            .expect("a legitimate reply still parses")
        else {
            panic!("expected a mount");
        };
        assert_eq!(m.user, "kdeconnect");
        assert_eq!(m.path, "/storage/emulated/0");
    }

    #[test]
    fn a_non_sftp_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_sftp(&ping).is_none());
    }
}
