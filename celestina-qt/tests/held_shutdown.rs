//! A helper restart must release every session-wide process it started.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(5);

struct Fixture {
    directory: PathBuf,
    helper: Option<Child>,
}

impl Fixture {
    fn start() -> Self {
        let directory =
            std::env::temp_dir().join(format!("celestina-held-shutdown-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("the fixture directory exists");
        let holder = directory.join("systemd-inhibit");
        let pid_file = directory.join("holder.pid");
        fs::write(
            &holder,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nexec sleep 30\n",
                pid_file.display()
            ),
        )
        .expect("the fake holder is written");
        let mut permissions = fs::metadata(&holder)
            .expect("the fake holder has metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&holder, permissions).expect("the fake holder is executable");

        let path = format!("{}:/usr/bin:/bin", directory.display());
        let helper = Command::new(env!("CARGO_BIN_EXE_celestina-provider-adapter"))
            .env("PATH", path)
            .env("XDG_CONFIG_HOME", &directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the provider helper starts");
        Self {
            directory,
            helper: Some(helper),
        }
    }

    fn holder_pid(&self) -> u32 {
        let pid_file = self.directory.join("holder.pid");
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Ok(text) = fs::read_to_string(&pid_file) {
                return text.trim().parse().expect("the holder writes its pid");
            }
            assert!(Instant::now() < deadline, "the hold was never started");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn helper_mut(&mut self) -> &mut Child {
        self.helper.as_mut().expect("the helper is still owned")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(mut helper) = self.helper.take() {
            let _ = helper.kill();
            let _ = helper.wait();
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn sigterm_releases_a_held_child_before_the_helper_exits() {
    let mut fixture = Fixture::start();
    // Providers register on their own startup threads. The real host receives
    // an initial frame before it can send an interactive command; preserve
    // that ordering here without coupling this lifecycle test to frame JSON.
    thread::sleep(Duration::from_millis(500));
    fixture
        .helper_mut()
        .stdin
        .as_mut()
        .expect("the helper stdin stays open")
        .write_all(
            b"{\"id\":\"1\",\"provider\":\"caffeine\",\"verb\":\"caffeine-on\",\"options\":{}}\n",
        )
        .expect("the hold request is sent");
    let holder_pid = fixture.holder_pid();

    let status = Command::new("kill")
        .args(["-TERM", &fixture.helper_mut().id().to_string()])
        .status()
        .expect("SIGTERM can be sent");
    assert!(status.success());
    let helper_status = fixture.helper_mut().wait().expect("the helper exits");
    assert!(helper_status.success());

    let deadline = Instant::now() + DEADLINE;
    while PathBuf::from(format!("/proc/{holder_pid}")).exists() {
        assert!(
            Instant::now() < deadline,
            "the held child survived its helper"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
