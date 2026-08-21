mod config;
mod daemon;
mod files;
mod mime;
mod rules;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "mime" {
        if args.len() < 3 {
            eprintln!("usage: harbor mime <file>...");
            std::process::exit(1);
        }
        for file in &args[2..] {
            let p = std::path::Path::new(file);
            match mime::detect(p) {
                Some(m) => println!("{file}: {m}"),
                None => println!("{file}: unknown"),
            }
        }
        return;
    }

    let path = config_path();
    let cfg = config::load(&path).unwrap_or_else(|e| {
        eprintln!("harbor: {e}");
        std::process::exit(1);
    });
    let n = cfg.folders.len();
    daemon::start(cfg.folders).unwrap_or_else(|e| {
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
