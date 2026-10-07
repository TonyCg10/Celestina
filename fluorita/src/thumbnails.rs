//! The thumbnail provider's seam, and the tests that keep it honest.
//!
//! The provider is `fluorita-qt`'s hand-written C++: CXX-Qt cannot subclass a
//! `QQuickAsyncImageProvider`. This module binds what the application calls —
//! registering it before the QML loads, draining its pool on quit — and three
//! helpers that only the tests below use, so the parts that can go wrong
//! without a window are checked without one: the cache key against its Rust
//! owner, the path a published key resolves to, and the decode with its
//! budgets.
//!
//! The provider generates image thumbnails only, through Qt's image reader.
//! Video posters and covers come from the engine's artwork pass, into the
//! same cache; nothing here starts the media backend.

// The three helpers exist for the tests below; the release binary only
// registers the provider and drains it.
#![cfg_attr(not(test), allow(dead_code))]

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("cxx-qt-lib/qsize.h");
        type QSize = cxx_qt_lib::QSize;
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("fluorita/thumbnailprovider.h");

        /// Adds the provider under "thumb". Before the QML that names
        /// `image://thumb/…` is loaded.
        #[rust_name = "register_thumbnail_provider"]
        fn register_fluorita_thumbnail_provider(engine: Pin<&mut QQmlApplicationEngine>);

        /// Drops the decodes not started and waits at most `milliseconds` for
        /// the running ones. Once, after the event loop has returned.
        #[rust_name = "thumbnail_shutdown"]
        fn fluorita_thumbnail_shutdown(milliseconds: i32);

        /// The cache key the provider computes for these raw path bytes.
        #[rust_name = "cache_uri"]
        fn fluorita_thumbnail_cache_uri(path_bytes: &QByteArray) -> QByteArray;

        /// The path bytes the provider resolves for a published key, reached
        /// through the same `image://thumb/<key>` URL a delegate writes.
        #[rust_name = "resolved_path"]
        fn fluorita_thumbnail_resolved_path(key: &QByteArray) -> QByteArray;

        /// The size of the thumbnail the provider would generate, without
        /// touching the shared cache. Invalid when a guard refuses.
        #[rust_name = "generated_size"]
        fn fluorita_thumbnail_generated_size(path_bytes: &QByteArray) -> QSize;
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{cache_uri, generated_size, resolved_path};
    use celestina_core::{pathkey, percent};
    use cxx_qt_lib::QByteArray;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};

    /// A temporary directory that removes itself.
    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-thumb-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir(&path).expect("fixture directory");
            Self(path)
        }

        /// Writes `contents` under a name given as raw bytes and answers with
        /// the bytes of the absolute path.
        fn write(&self, name: &[u8], contents: &[u8]) -> Vec<u8> {
            let file = self.0.join(OsString::from_vec(name.to_vec()));
            std::fs::write(&file, contents).expect("fixture file");
            percent::path_bytes(&file)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An uncompressed 24-bit BMP of `width` by `height`, written by hand so
    /// the fixture needs no encoder. `rows` limits how many pixel rows are
    /// actually present, so a header can claim more than the file holds.
    fn bmp(width: u32, height: u32, rows: u32) -> Vec<u8> {
        let stride = (width * 3).div_ceil(4) * 4;
        let pixels = stride * height;
        let mut file = Vec::new();
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(54 + pixels).to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&54u32.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&width.to_le_bytes());
        file.extend_from_slice(&height.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&24u16.to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&pixels.to_le_bytes());
        file.extend_from_slice(&2835u32.to_le_bytes());
        file.extend_from_slice(&2835u32.to_le_bytes());
        file.extend_from_slice(&[0; 8]);
        file.resize(file.len() + (stride * rows) as usize, 0x80);
        file
    }

    fn size_of(path: &[u8]) -> (i32, i32) {
        let size = generated_size(&QByteArray::from(path));
        (size.width(), size.height())
    }

    #[test]
    fn the_provider_keys_the_cache_where_the_engine_looks() {
        // The grid now asks the provider for an image and the projection
        // still finds video posters by `large_thumbnail_path`; both have to
        // name the same entry or one of them stops reusing the other's work.
        for path in [
            "/home/toni/photo.png",
            "/home/toni/my photos/photo (1).jpeg",
            "/home/toni/plus+amp&eq=semi;at@colon:comma,.png",
            "/home/toni/accented caf\u{e9} \u{f1}.png",
        ] {
            let produced = Vec::<u8>::from(&cache_uri(&QByteArray::from(path.as_bytes())));
            let expected = fluorita_core::file_uri(Path::new(path)).expect("absolute");
            assert_eq!(produced, expected.into_bytes(), "{path}");
        }
        let produced = Vec::<u8>::from(&cache_uri(&QByteArray::from(&b"/m/na\xffme.png"[..])));
        assert_eq!(produced, b"file:///m/na%FFme.png".to_vec());
    }

    #[test]
    fn a_published_key_resolves_to_the_bytes_it_was_made_from() {
        // Qt hands the provider an id it has already PrettyDecoded; an
        // accented name arrives as characters and a byte that is not UTF-8
        // arrives still escaped. Both must come back as the file's bytes.
        for name in [
            b"/home/toni/photo.png".to_vec(),
            "/home/toni/Pictures/caf\u{e9} \u{f1}.jpg"
                .as_bytes()
                .to_vec(),
            b"/home/toni/my photos/a (1) #2 100%.png".to_vec(),
            b"/home/toni/na\xffme.png".to_vec(),
        ] {
            let key = pathkey::encode(Path::new(&OsString::from_vec(name.clone())));
            let resolved = Vec::<u8>::from(&resolved_path(&QByteArray::from(key.as_bytes())));
            assert_eq!(resolved, name, "{key}");
        }
    }

    #[test]
    fn a_large_image_is_decoded_down_to_the_thumbnail_box() {
        let fixture = Fixture::new("downscale");
        let wide = fixture.write(b"panorama.bmp", &bmp(600, 300, 300));
        let small = fixture.write(b"na\xffme.bmp", &bmp(40, 20, 20));

        assert_eq!(size_of(&wide), (256, 128), "the long side is the box");
        assert_eq!(size_of(&small), (40, 20), "a small image is never enlarged");
    }

    #[test]
    fn the_budgets_and_guards_refuse_before_anything_is_decoded() {
        let fixture = Fixture::new("guards");
        // A header claiming 400 megapixels over a few bytes: past the budget,
        // so it is refused from the header alone.
        let enormous = fixture.write(b"huge.bmp", &bmp(20_000, 20_000, 0));
        let not_an_image = fixture.write(b"note.txt", b"hello");
        let directory = percent::path_bytes(&fixture.0);
        let missing = [directory.as_slice(), b"/absent.png"].concat();

        for (label, bytes) in [
            ("a claimed surface past the budget", enormous),
            ("a file that is not an image", not_an_image),
            ("a directory", directory.clone()),
            ("a file that is not there", missing),
            ("a relative path", b"relative.png".to_vec()),
        ] {
            let (width, _) = size_of(&bytes);
            assert!(width <= 0, "{label} was accepted");
        }
    }
}
