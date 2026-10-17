//! How long one scan of the host takes (task T2.7): `cargo run --release -p devports-core --example scan_cost`.

#![allow(clippy::print_stdout)]

#[cfg(target_os = "macos")]
fn main() {
    use devports_core::platform::macos::MacProcessSource;
    use devports_core::process::ProcessSource;

    let Ok(source) = MacProcessSource::new() else {
        println!("cannot read the process table");
        return;
    };
    let source = if std::env::args().any(|argument| argument == "--skip-system") {
        source.skipping_system_programs()
    } else {
        source
    };
    let mut times = Vec::new();
    let mut found = 0;
    let mut table = 0;
    for _ in 0..20 {
        let started = std::time::Instant::now();
        found = source.snapshot().map_or(0, |scan| scan.len());
        times.push(started.elapsed());
        table = source.all_processes().map_or(0, |all| all.len());
    }
    times.sort();
    println!(
        "{table} processes, {found} listening; scan median {:?}, worst {:?}",
        times.get(times.len() / 2).copied().unwrap_or_default(),
        times.last().copied().unwrap_or_default()
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}
