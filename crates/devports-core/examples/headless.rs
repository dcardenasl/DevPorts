//! Runs the whole core with no interface, to try the CLI against it:
//!
//! ```text
//! cargo run -p devports-core --example headless
//! cargo run -p devports-cli -- ls
//! ```
//!
//! Stops on Ctrl-C and removes its socket.

// An example talks to the terminal.
#![allow(clippy::print_stdout, clippy::print_stderr)]

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() {
    use devports_core::runtime::{Core, CoreOptions};

    let Some(options) = CoreOptions::from_env() else {
        eprintln!("cannot tell where to put the files: set HOME");
        std::process::exit(1);
    };
    println!("socket: {}", options.socket.display());
    let core = match Core::start_system(options).await {
        Ok(core) => core,
        Err(error) => {
            eprintln!("cannot start: {error}");
            std::process::exit(1);
        }
    };
    let handle = core.handle();
    let mut notices = handle.notices();
    tokio::spawn(async move {
        while let Ok(notice) = notices.recv().await {
            println!("notice: {notice:?}");
        }
    });
    let _ = tokio::signal::ctrl_c().await;
    handle.shutdown();
    core.wait().await;
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("DevPorts runs on macOS only");
}
