//! Project stops over the real macOS host, using disposable listeners only.

#![cfg(target_os = "macos")]
#![allow(clippy::disallowed_methods, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use devports_core::engine::StopExpectation;
use devports_core::runtime::{Core, CoreOptions};
use tempfile::TempDir;

struct Listener(Child);

impl Listener {
    fn start(cwd: &Path) -> Self {
        std::fs::create_dir_all(cwd).expect("folder");
        let mut child = Command::new("python3")
            .args(["-u", "-c", "import socket,time; s=socket.socket(); s.bind(('127.0.0.1',0)); s.listen(); print(s.getsockname()[1],flush=True); time.sleep(300)"])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn().expect("listener starts");
        let mut line = String::new();
        BufReader::new(child.stdout.take().expect("stdout"))
            .read_line(&mut line)
            .expect("ready");
        assert!(
            line.trim().parse::<u16>().is_ok(),
            "listener reports its port"
        );
        Self(child)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// covers: UXR-2.2
#[tokio::test]
async fn confirmed_project_applications_stop_but_data_and_later_arrivals_survive() {
    let home = TempDir::new().expect("home");
    let root = home.path().join("work");
    let project = root.join("project");
    std::fs::create_dir_all(home.path().join(".config/devports")).expect("config folder");
    std::fs::create_dir_all(&project).expect("project");
    std::fs::write(
        home.path().join(".config/devports/config.yml"),
        format!(
            "version: 1\nroots: [{}]\nobservation:\n  host_interval: 250ms\n",
            root.display()
        ),
    )
    .expect("config");
    std::fs::write(project.join(".devports.yml"), "version: 1\nproject: controlled\nservices:\n  api:\n    cwd: ./api\n  web:\n    cwd: ./web\n  database:\n    cwd: ./database\n    kind: data\n  later:\n    cwd: ./later\n").expect("manifest");
    let mut api = Listener::start(&project.join("api"));
    let mut web = Listener::start(&project.join("web"));
    let mut data = Listener::start(&project.join("database"));
    let core = Core::start_system(CoreOptions {
        home: home.path().to_path_buf(),
        database: home.path().join("db"),
        socket: home.path().join("s/d.sock"),
    })
    .await
    .expect("isolated core");
    let handle = core.handle();
    let mut preview = Vec::new();
    for _ in 0..100 {
        preview = handle
            .preview_project_stop("controlled")
            .await
            .expect("preview");
        if preview.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(preview.len(), 2);
    assert!(
        preview
            .iter()
            .all(|view| view.kind == "app" && !view.shared)
    );
    let mut later = Listener::start(&project.join("later"));
    // Even after observation catches up, executing the preview cannot expand it.
    for _ in 0..100 {
        if handle
            .preview_project_stop("controlled")
            .await
            .expect("later preview")
            .len()
            == 3
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        handle
            .preview_project_stop("controlled")
            .await
            .expect("later preview")
            .len(),
        3
    );
    for view in preview {
        tokio::time::timeout(
            Duration::from_secs(10),
            handle.stop_project(&view.instance_ref, StopExpectation::from_view(&view)),
        )
        .await
        .expect("termination completes")
        .expect("stopped");
    }
    assert!(api.0.try_wait().expect("api status").is_some());
    assert!(web.0.try_wait().expect("web status").is_some());
    assert!(data.0.try_wait().expect("data status").is_none());
    assert!(later.0.try_wait().expect("later status").is_none());
    handle.shutdown();
    core.wait().await;
    // RAII reaps both stopped children and cleans up the surviving test listeners.
}
