//! `/etc/passwd`, for the one thing the table needs from it: a login name
//! for a uid. Anything else on a line is ignored; a malformed line is skipped
//! rather than refused, because one bad entry must not blank every user.

use std::collections::HashMap;

#[must_use]
pub fn parse(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let _password = fields.next()?;
            let uid = fields.next()?.parse::<u32>().ok()?;
            (!name.is_empty()).then(|| (uid, name.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn names_are_keyed_by_uid_and_bad_lines_are_skipped() {
        let users = parse("root:x:0:0:root:/root:/bin/bash\ntoni:x:1000:1000::/home/toni:/bin/zsh\nbroken\n:x:5:5::/:/bin/false\n");
        assert_eq!(users.get(&0).map(String::as_str), Some("root"));
        assert_eq!(users.get(&1000).map(String::as_str), Some("toni"));
        assert_eq!(users.len(), 2);
    }
}
