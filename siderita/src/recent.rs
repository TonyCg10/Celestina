use std::path::PathBuf;

/// One entry of the desktop's recently-used list: where it is and when it was
/// last touched (the raw ISO-8601 stamp the file carries — it sorts correctly
/// as text, so nothing needs parsing to order the list).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentItem {
    pub path: PathBuf,
    pub name: String,
    pub stamp: String,
}

/// Reads the freedesktop recently-used list, newest first, keeping only entries
/// that still exist and at most `limit` of them.
///
/// This is interop, not an index: the file is the desktop's, written by every
/// application that opens something, and Siderita only reads it. Anything it
/// cannot parse is skipped rather than guessed at.
pub fn load(limit: usize) -> Vec<RecentItem> {
    let Some(path) = celestina_core::xdg::data_home().map(|dir| dir.join("recently-used.xbel")) else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut items = parse(&content);
    // Newest first, and the name breaks ties so the order never wobbles between
    // two entries written in the same instant.
    items.sort_by(|left, right| {
        right
            .stamp
            .cmp(&left.stamp)
            .then_with(|| left.name.cmp(&right.name))
    });
    items.retain(|item| item.path.exists());
    items.truncate(limit);
    items
}

/// Pulls the `href` and the most recent timestamp out of every `<bookmark>` tag.
/// A deliberately small scanner: XBEL is a fixed, machine-written format here,
/// and a real XML parser is a dependency this app does not need.
fn parse(content: &str) -> Vec<RecentItem> {
    let mut items = Vec::new();
    for chunk in content.split("<bookmark ").skip(1) {
        let Some(tag_end) = chunk.find('>') else {
            continue;
        };
        let tag = &chunk[..tag_end];
        let Some(href) = attribute(tag, "href") else {
            continue;
        };
        let Some(path) = crate::dbus::uri_to_path(&href) else {
            continue;
        };
        let stamp = attribute(tag, "visited")
            .into_iter()
            .chain(attribute(tag, "modified"))
            .chain(attribute(tag, "added"))
            .max()
            .unwrap_or_default();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        items.push(RecentItem { path, name, stamp });
    }
    items
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0">
  <bookmark href="file:///tmp/a%20b.txt" added="2026-01-01T00:00:00Z" modified="2026-02-01T00:00:00Z" visited="2026-03-01T00:00:00Z">
    <info/>
  </bookmark>
  <bookmark href="file:///tmp/older.txt" added="2025-01-01T00:00:00Z" modified="2025-01-01T00:00:00Z" visited="2025-01-01T00:00:00Z">
    <info/>
  </bookmark>
  <bookmark href="sftp://elsewhere/nope.txt" visited="2027-01-01T00:00:00Z"/>
  <bookmark broken
</xbel>"#;

    #[test]
    fn parses_local_bookmarks_and_percent_decodes_them() {
        let items = parse(SAMPLE);
        assert_eq!(items.len(), 2, "the remote and the truncated tag are skipped");
        assert_eq!(items[0].path, PathBuf::from("/tmp/a b.txt"));
        assert_eq!(items[0].name, "a b.txt");
    }

    #[test]
    fn the_newest_timestamp_of_a_bookmark_wins() {
        let items = parse(SAMPLE);
        assert_eq!(items[0].stamp, "2026-03-01T00:00:00Z");
    }

    #[test]
    fn a_missing_attribute_is_not_fatal() {
        let items = parse(r#"<bookmark href="file:///tmp/x"><info/>"#);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stamp, "");
    }

    #[test]
    fn nothing_parses_out_of_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("<xbel></xbel>").is_empty());
    }
}
