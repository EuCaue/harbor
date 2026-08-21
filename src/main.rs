mod config;
mod daemon;
mod files;
mod mime;
mod rules;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        match args[1].as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return;
            }
            "-v" | "--version" | "version" => {
                println!("harbor {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "mime" => {
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
            _ => {}
        }
    }

    let (path, is_default) = config_path();
    if is_default && !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, config::DEFAULT_CONFIG_TEMPLATE).is_ok() {
            println!("harbor: created initial config at {}", path.display());
            println!("harbor: edit your rules and run harbor again to start.");
            return;
        }
    }

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

fn config_path() -> (PathBuf, bool) {
    match std::env::args().nth(1) {
        Some(p) => (PathBuf::from(p), false),
        None => {
            let p = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                PathBuf::from(xdg).join("harbor/config.toml")
            } else if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(".config/harbor/config.toml")
            } else if let Ok(profile) = std::env::var("USERPROFILE") {
                PathBuf::from(profile).join(".config/harbor/config.toml")
            } else {
                PathBuf::from("config.toml")
            };
            (p, true)
        }
    }
}

fn print_help() {
    println!(
        "harbor {} - zero-async file organizer daemon\n\n\
        USAGE:\n    \
            harbor [OPTIONS] [CONFIG_PATH]\n    \
            harbor mime <FILE>...\n\n\
        COMMANDS:\n    \
            mime <FILE>...    Inspect detected MIME type of files\n\n\
        OPTIONS:\n    \
            -h, --help        Print help information\n    \
            -v, --version     Print version information",
        env!("CARGO_PKG_VERSION")
    );
}
