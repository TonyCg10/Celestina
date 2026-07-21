use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use siderita_core::{ScanCoordinator, ScanExecutor};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "celestina-latest-scan-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        fs::write(path.join(label), label.as_bytes()).expect("write fixture");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn latest_navigation_eventually_publishes_under_burst_load() {
    let first = TestDirectory::new("first");
    let second = TestDirectory::new("second");
    let latest = TestDirectory::new("latest");
    let mut coordinator = ScanCoordinator::new();
    let first_request = coordinator.begin(first.path()).expect("first request");
    let second_request = coordinator.begin(second.path()).expect("second request");
    let latest_request = coordinator.begin(latest.path()).expect("latest request");
    let latest_generation = latest_request.generation();
    let (sender, receiver) = mpsc::channel();
    let executor = ScanExecutor::new(move |result| {
        let _ = sender.send(result);
    });

    executor.submit(first_request).expect("submit first");
    executor.submit(second_request).expect("submit second");
    executor.submit(latest_request).expect("submit latest");

    let published = loop {
        let result = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("scan result");
        if let Ok(snapshot) = result {
            if snapshot.generation() == latest_generation {
                break snapshot;
            }
        }
    };

    assert_eq!(published.location(), latest.path());
    assert_eq!(published.entries().len(), 1);
}
