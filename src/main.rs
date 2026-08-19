mod config;
mod daemon;
mod files;
mod rules;

use std::path::PathBuf;

fn main() {
    let path = config_path();
    let cfg = config::load(&path).unwrap_or_else(|e| {
        eprintln!("harbor: {e}");
        std::process::exit(1);
    });
    let n = cfg.watches.len();
    daemon::start(cfg.watches).unwrap_or_else(|e| {
        eprintln!("harbor: {e}");
        std::process::exit(1);
    });
    println!("harbor: watching {n} dir(s)");
    std::thread::park();
}

fn config_path() -> PathBuf {
    match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".config/harbor/config.toml"))
            .unwrap_or_else(|_| PathBuf::from("config.toml")),
    }
}
