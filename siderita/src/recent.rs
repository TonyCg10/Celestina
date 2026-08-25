use std::path::{Path, PathBuf};

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
///
/// Blocking, and deliberately so — its caller reads it on a worker thread.
pub fn load(limit: usize) -> Vec<RecentItem> {
    let Some(path) = celestina_core::xdg::data_home().map(|dir| dir.join("recently-used.xbel"))
    else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    newest_existing(parse(&content), limit, |path| path.exists())
}

/// The newest `limit` entries that still exist, newest first.
///
/// `exists` is a parameter because the cost of this function *is* how often it
/// is called: one filesystem round trip each, against a list that records
/// everything the desktop has ever opened — including files on a phone or a
/// share that has since stopped answering, where a single question can block
/// for as long as that filesystem takes to give up. Asking about every entry
/// and then throwing all but the newest `limit` away paid that price for
/// answers nobody would read, so the list is ordered first and asked about one
/// entry at a time until it is full.
fn newest_existing(
    mut items: Vec<RecentItem>,
    limit: usize,
    mut exists: impl FnMut(&Path) -> bool,
) -> Vec<RecentItem> {
    // Newest first, and the name breaks ties so the order never wobbles between
    // two entries written in the same instant.
    items.sort_by(|left, right| {
        right
            .stamp
            .cmp(&left.stamp)
            .then_with(|| left.name.cmp(&right.name))
    });
    let mut kept = Vec::with_capacity(limit.min(items.len()));
    for item in items {
        if kept.len() == limit {
            break;
        }
        if exists(&item.path) {
            kept.push(item);
        }
    }
    kept
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
    Some(unescape(&rest[..end]))
}

/// Resolves the five XML entities an attribute value may carry.
///
/// The writers of this file escape `&` as `&amp;`, so a `href` for a file whose
/// name contains an ampersand reaches here as `%26` inside `&amp;…` — and
/// leaving the entity in place made the URI decode to a path that does not
/// exist, which `load` then filtered out. A file with an `&` in its name simply
/// never appeared in Recientes.
///
/// One pass, so a value that decodes to `&lt;` is not decoded twice.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let entity = tail
            .find(';')
            .map(|end| &tail[..=end])
            .filter(|entity| entity.len() <= 6);
        let resolved = match entity {
            Some("&amp;") => Some('&'),
            Some("&lt;") => Some('<'),
            Some("&gt;") => Some('>'),
            Some("&quot;") => Some('"'),
            Some("&apos;") => Some('\''),
            _ => None,
        };
        match resolved {
            // An unknown entity is left exactly as written: guessing at it would
            // invent a path.
            Some(character) => {
                out.push(character);
                rest = &tail[entity.map_or(1, str::len)..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
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
        assert_eq!(
            items.len(),
            2,
            "the remote and the truncated tag are skipped"
        );
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
    fn an_escaped_ampersand_in_a_name_still_resolves_to_its_file() {
        // How every writer of this file spells `rock & roll.mp3`.
        let items = parse(
            r#"<bookmark href="file:///tmp/rock%20%26%20roll.mp3" visited="2026-01-01T00:00:00Z"/>"#,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, PathBuf::from("/tmp/rock & roll.mp3"));
        assert_eq!(items[0].name, "rock & roll.mp3");
    }

    #[test]
    fn the_other_four_entities_resolve_and_an_unknown_one_is_left_alone() {
        assert_eq!(
            super::unescape("a &lt;b&gt; &quot;c&quot; &apos;d&apos;"),
            "a <b> \"c\" 'd'"
        );
        // Written by a file called `&`, not an entity.
        assert_eq!(super::unescape("&amp;amp;"), "&amp;");
        assert_eq!(super::unescape("100 &euro; & more"), "100 &euro; & more");
        assert_eq!(super::unescape("nothing to do"), "nothing to do");
    }

    /// Builds `count` entries, newest first by stamp, named `0.txt`, `1.txt`, …
    fn stamped(count: usize) -> Vec<RecentItem> {
        (0..count)
            .map(|index| RecentItem {
                path: PathBuf::from(format!("/tmp/{index}.txt")),
                name: format!("{index}.txt"),
                // Zero-padded and descending, because the stamp is compared as
                // text: an unpadded number would order 99 above 500.
                stamp: format!("2026-01-01T00:00:00.{:04}Z", count - index),
            })
            .collect()
    }

    #[test]
    fn it_stops_asking_the_filesystem_once_the_list_is_full() {
        let mut asked = 0;
        let kept = super::newest_existing(stamped(500), 100, |_| {
            asked += 1;
            true
        });
        assert_eq!(kept.len(), 100);
        assert_eq!(
            asked, 100,
            "a full list must not cost a question per recorded entry"
        );
        assert_eq!(kept[0].name, "0.txt", "still newest first");
    }

    #[test]
    fn it_keeps_asking_past_entries_that_are_gone() {
        let mut asked = 0;
        // Only every third entry still exists, so filling a list of two takes
        // six questions — the cost follows what is missing, never the whole file.
        let kept = super::newest_existing(stamped(30), 2, |_| {
            asked += 1;
            asked % 3 == 0
        });
        assert_eq!(kept.len(), 2);
        assert_eq!(asked, 6);
        assert_eq!(kept[0].name, "2.txt");
        assert_eq!(kept[1].name, "5.txt");
    }

    #[test]
    fn a_limit_of_nothing_asks_nothing() {
        let mut asked = 0;
        let kept = super::newest_existing(stamped(10), 0, |_| {
            asked += 1;
            true
        });
        assert!(kept.is_empty());
        assert_eq!(asked, 0);
    }

    #[test]
    fn a_short_list_is_returned_whole_and_ordered() {
        let kept = super::newest_existing(stamped(3), 100, |_| true);
        let names: Vec<&str> = kept.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["0.txt", "1.txt", "2.txt"]);
    }

    #[test]
    fn nothing_parses_out_of_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("<xbel></xbel>").is_empty());
    }
}
