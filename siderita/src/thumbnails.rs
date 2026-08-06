//! The thumbnail seam's one rule, and the test that keeps it true.
//!
//! The provider itself is hand-written C++ (`cpp/thumbnailprovider.cpp`),
//! because cxx-qt exposes no image-provider hook. Nothing in Rust computes a
//! thumbnail, so this module binds exactly one of its functions: the
//! freedesktop cache key.
//!
//! That key is not Qt plumbing but a shared spelling. The cache under
//! `~/.cache/thumbnails/` is keyed on the MD5 of a `file://` URI, and every
//! desktop file manager writes into the same directory, so a byte of
//! disagreement means Siderita silently stops reusing — and contributing to —
//! what the rest of the session already produced. The owner of that spelling is
//! [`celestina_core::percent::encode_qt_path`]; the provider reproduces it over
//! raw path bytes, since a name that is not valid UTF-8 has no QString to hand
//! to `QUrl::fromLocalFile`. Two implementations of one rule are exactly what
//! the reuse rule warns about, so the equality is asserted here rather than
//! asserted in a comment.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;

        include!("siderita/thumbnailprovider.h");

        include!("cxx-qt-lib/qsize.h");
        type QSize = cxx_qt_lib::QSize;

        /// The cache key the C++ provider computes for these raw path bytes.
        #[rust_name = "cache_uri"]
        fn siderita_thumbnail_cache_uri(path_bytes: &QByteArray) -> QByteArray;

        /// The pixel size of the image at these raw path bytes, read through
        /// the provider's own guards and descriptor. Invalid when they refuse.
        #[rust_name = "source_size"]
        fn siderita_thumbnail_source_size(path_bytes: &QByteArray) -> QSize;

        /// The path bytes the provider resolves for a published key, reached
        /// through the same `image://thumb/<key>` URL a delegate writes.
        #[rust_name = "resolved_path"]
        fn siderita_thumbnail_resolved_path(key: &QByteArray) -> QByteArray;
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{cache_uri, resolved_path, source_size};
    use celestina_core::percent;
    use cxx_qt_lib::QByteArray;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A 2x2 red PNG, written byte by byte so the fixture needs no encoder.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd,
        0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x9f, 0x01, 0x8c, 0xff, 0x33, 0x30, 0x00, 0x00, 0x1f, 0xee, 0x03, 0xfd,
        0x35, 0x1b, 0x00, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
        0x82,
    ];

    /// A temporary directory that removes itself, holding image fixtures whose
    /// names the test chooses byte by byte.
    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-thumb-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create fixture directory");
            Self(path)
        }

        /// Writes `contents` under a name given as raw bytes, and answers with
        /// the bytes of the resulting absolute path.
        fn write(&self, name: &[u8], contents: &[u8]) -> Vec<u8> {
            let file = self.0.join(OsString::from_vec(name.to_vec()));
            fs::write(&file, contents).expect("write fixture file");
            percent::path_bytes(&file)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// What the owner of the spelling says the key for `bytes` is.
    fn expected(bytes: &[u8]) -> String {
        format!("file://{}", percent::encode_qt_path(bytes))
    }

    /// The C++ provider's answer for the same bytes.
    fn produced(bytes: &[u8]) -> Vec<u8> {
        Vec::<u8>::from(&cache_uri(&QByteArray::from(bytes)))
    }

    /// What the provider actually resolves for a key a delegate published.
    fn resolved(bytes: &[u8]) -> Vec<u8> {
        let key = percent::encode_qt_path(bytes);
        Vec::<u8>::from(&resolved_path(&QByteArray::from(key.as_bytes())))
    }

    #[test]
    fn a_key_survives_the_url_qt_hands_the_provider() {
        // Qt derives the provider's id from the URL with PrettyDecoded
        // formatting, so an escape spelling valid UTF-8 arrives already decoded
        // and one that does not arrives still escaped. Both have to come back
        // as the bytes that name the file, and the accented case is the one a
        // key-only test cannot see: it was broken for a whole delivery while
        // every direct test stayed green.
        for name in [
            b"/home/toni/foto.png".to_vec(),
            "/home/toni/ni\u{f1}o.png".as_bytes().to_vec(),
            "/home/toni/Im\u{e1}genes/fotograf\u{ed}a.jpg"
                .as_bytes()
                .to_vec(),
            "/home/toni/\u{1f4c1}/a.png".as_bytes().to_vec(),
            b"/home/toni/mis fotos/a (1).png".to_vec(),
            b"/home/toni/na\xffme.png".to_vec(),
        ] {
            assert_eq!(
                resolved(&name),
                name,
                "resolving {:?} through the provider URL",
                String::from_utf8_lossy(&name)
            );
        }
    }

    #[test]
    fn the_provider_and_the_codec_spell_an_ordinary_name_identically() {
        for path in [
            "/home/toni/foto.png",
            "/home/toni/mis fotos/foto (1).jpeg",
            "/home/toni/plus+amp&eq=semi;at@colon:comma,.png",
            "/home/toni/weird#hash?q.png",
            "/home/toni/acentuado \u{e1}\u{f1}.png",
        ] {
            assert_eq!(
                produced(path.as_bytes()),
                expected(path.as_bytes()).into_bytes(),
                "the two spellings of {path} diverged"
            );
        }
    }

    #[test]
    fn a_name_that_is_not_utf8_gets_a_key_instead_of_none() {
        // The case the whole seam exists for. A byte fixture, not text, so the
        // language contract does not apply.
        let bytes = b"/home/toni/na\xffme.png";
        let key = produced(bytes);
        assert!(!key.is_empty(), "a non-UTF-8 name must still key the cache");
        assert_eq!(key, b"file:///home/toni/na%FFme.png".to_vec());
        assert_eq!(key, expected(bytes).into_bytes());
    }

    #[test]
    fn an_image_whose_name_is_not_utf8_is_still_found_and_decoded() {
        let fixture = Fixture::new("nonutf8");
        // The name a QString cannot hold; its extension is ordinary.
        let path = fixture.write(b"na\xffme.png", TINY_PNG);

        let size = source_size(&QByteArray::from(path.as_slice()));
        assert_eq!(
            (size.width(), size.height()),
            (2, 2),
            "the provider found and decoded the file its name used to hide"
        );
    }

    #[test]
    fn the_guards_refuse_what_the_provider_would_not_generate_from() {
        let fixture = Fixture::new("guards");
        let directory = percent::path_bytes(&fixture.0);
        let not_an_image = fixture.write(b"na\xffme.txt", b"hola");
        let missing = [directory.as_slice(), b"/ausente.png"].concat();

        for (label, bytes) in [
            ("a directory", directory.clone()),
            ("a file that is not a generatable image", not_an_image),
            ("a file that is not there", missing),
            ("a relative path", b"relativo.png".to_vec()),
        ] {
            let size = source_size(&QByteArray::from(bytes.as_slice()));
            assert!(!size.width().is_positive(), "{label} was accepted");
        }
    }
}
