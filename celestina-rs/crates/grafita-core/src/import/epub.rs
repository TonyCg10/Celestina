//! Which XHTML documents an EPUB is made of, and in which order.
//!
//! A book's chapters are separate files, and only the package document says
//! which they are and how they follow one another. Reading them in the order
//! the archive happens to store them would present a book in an order nobody
//! wrote, so the spine is read even though it means two more lookups.

use crate::container::Container;

use super::ImportError;

/// The reading order: every content document of the spine, in its order.
pub(super) fn reading_order(container: &Container) -> Result<Vec<String>, ImportError> {
    let container_xml = container
        .read("META-INF/container.xml")
        .map_err(ImportError::Container)?;
    let container_xml = String::from_utf8(container_xml).map_err(|_| ImportError::Incomplete {
        detail: "its container description is not UTF-8".to_owned(),
    })?;
    let package_path = attribute(&container_xml, "rootfile", "full-path").ok_or_else(|| {
        ImportError::Incomplete {
            detail: "it names no package document".to_owned(),
        }
    })?;

    let package = container
        .read(&package_path)
        .map_err(ImportError::Container)?;
    let package = String::from_utf8(package).map_err(|_| ImportError::Incomplete {
        detail: "its package document is not UTF-8".to_owned(),
    })?;
    let base = package_path
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);

    let manifest = manifest(&package);
    let mut order = Vec::new();
    for reference in spine(&package) {
        let Some(href) = manifest
            .iter()
            .find_map(|(id, href)| (*id == reference).then(|| (*href).to_owned()))
        else {
            return Err(ImportError::Incomplete {
                detail: format!("its spine names '{reference}', which the manifest does not hold"),
            });
        };
        let path = if base.is_empty() {
            href
        } else {
            format!("{base}/{href}")
        };
        // A spine also carries covers and navigation documents; the ones this
        // editor shows are the ones that hold text, and a missing member is the
        // book's problem rather than a reason to refuse it.
        if container.names().contains(&path.as_str()) {
            order.push(path);
        }
    }
    if order.is_empty() {
        return Err(ImportError::Incomplete {
            detail: "its spine leads to no document".to_owned(),
        });
    }
    Ok(order)
}

/// The manifest as `(id, href)` pairs.
fn manifest(package: &str) -> Vec<(&str, &str)> {
    elements(package, "item")
        .filter_map(|element| {
            let id = attribute_of(element, "id")?;
            let href = attribute_of(element, "href")?;
            Some((id, href))
        })
        .collect()
}

/// The spine as the manifest identifiers it references, in order.
fn spine(package: &str) -> Vec<&str> {
    elements(package, "itemref")
        .filter_map(|element| attribute_of(element, "idref"))
        .collect()
}

/// Every element with this name, as the text inside its angle brackets.
fn elements<'a>(source: &'a str, name: &'a str) -> impl Iterator<Item = &'a str> {
    let mut cursor = 0;
    std::iter::from_fn(move || {
        while let Some(found) = source[cursor..].find('<') {
            let open = cursor + found;
            let close = source[open..].find('>').map(|offset| open + offset)?;
            let element = &source[open + 1..close];
            cursor = close + 1;
            let tag = element
                .trim_start_matches('/')
                .split([' ', '/', '\t', '\n', '\r'])
                .next()
                .unwrap_or("");
            // Namespace prefixes are allowed and ignored: a package may write
            // `opf:item` and mean `item`.
            let bare = tag.rsplit(':').next().unwrap_or(tag);
            if bare == name {
                return Some(element);
            }
        }
        None
    })
}

fn attribute(source: &str, element: &str, name: &str) -> Option<String> {
    elements(source, element)
        .find_map(|found| attribute_of(found, name))
        .map(str::to_owned)
}

/// One attribute's value, quoted with either quote character.
fn attribute_of<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let mut cursor = 0;
    while let Some(found) = element[cursor..].find(name) {
        let at = cursor + found;
        let before_is_boundary = at == 0
            || element[..at]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_whitespace());
        let rest = &element[at + name.len()..];
        let rest = rest.trim_start();
        if before_is_boundary && rest.starts_with('=') {
            let value = rest[1..].trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let end = value[1..].find(quote)? + 1;
                return Some(&value[1..end]);
            }
        }
        cursor = at + name.len();
    }
    None
}
