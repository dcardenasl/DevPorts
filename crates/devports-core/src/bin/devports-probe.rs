//! Phase 0 probe binary. See `devports_core::probe` for why it exists and when
//! it goes away.
//!
//! ```text
//! devports-probe ls      list the TCP listeners of the current user
//! devports-probe opens   show how many times `ls` was run per day
//! ```

// A command-line tool talks to the terminal; the workspace denies printing
// because the core library must never do it.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use devports_core::probe;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("ls") => list(),
        Some("opens") => opens(),
        _ => {
            eprintln!("uso: devports-probe ls | opens");
            ExitCode::from(16)
        }
    }
}

#[cfg(target_os = "macos")]
fn list() -> ExitCode {
    use devports_core::platform::macos::MacProcessSource;
    use devports_core::process::ProcessSource;

    let processes = MacProcessSource::new().and_then(|source| source.snapshot());
    match processes {
        Ok(processes) => {
            print!("{}", probe::render_table(&processes));
            // The open is recorded after a successful scan; a failed write must not
            // hide the result, so it only warns.
            if let Some(dir) = probe::baseline_dir()
                && let Err(error) = probe::record_open(&dir, "ls", &jiff::Zoned::now())
            {
                eprintln!("devports-probe: no se pudo registrar la apertura: {error}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("devports-probe: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn list() -> ExitCode {
    eprintln!("devports-probe: solo está disponible en macOS");
    ExitCode::FAILURE
}

fn opens() -> ExitCode {
    let Some(dir) = probe::baseline_dir() else {
        eprintln!("devports-probe: no se pudo determinar la carpeta de datos (HOME)");
        return ExitCode::FAILURE;
    };
    let log = std::fs::read_to_string(dir.join("opens.log")).unwrap_or_default();
    print!("{}", probe::render_opens(&probe::opens_per_day(&log)));
    ExitCode::SUCCESS
}
