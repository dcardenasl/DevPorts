//! How long one pass over the project roots takes (task T2.7):
//! `cargo run --release -p devports-core --example discovery_cost`.

#![allow(clippy::print_stdout)]

fn main() {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        println!("HOME is not set");
        return;
    };
    let roots = devports_core::discovery::default_roots(&home);
    let mut times = Vec::new();
    let mut projects = 0;
    for _ in 0..10 {
        let started = std::time::Instant::now();
        projects =
            devports_core::discovery::discover(&roots, devports_core::discovery::DEFAULT_MAX_DEPTH)
                .catalog
                .projects()
                .len();
        times.push(started.elapsed());
    }
    times.sort();
    println!(
        "{projects} projects; pass median {:?}, worst {:?}",
        times.get(times.len() / 2).copied().unwrap_or_default(),
        times.last().copied().unwrap_or_default()
    );
}
