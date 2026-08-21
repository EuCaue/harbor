mod config;
mod daemon;
mod files;
mod mime;
mod rules;

use std::path::PathBuf;

fn main() {
    let (path, is_default) = parse_args();
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

fn parse_args() -> (PathBuf, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut config: Option<PathBuf> = None;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "-v" | "--version" | "version" => {
                println!("harbor {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "mime" => {
                if i + 1 >= args.len() {
                    eprintln!("usage: harbor mime <file>...");
                    std::process::exit(1);
                }
                for file in &args[i + 1..] {
                    let p = std::path::Path::new(file);
                    match mime::detect(p) {
                        Some(m) => println!("{file}: {m}"),
                        None => println!("{file}: unknown"),
                    }
                }
                std::process::exit(0);
            }
            "-c" | "--config" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: flag '{arg}' requires a path argument");
                    std::process::exit(1);
                }
                config = Some(PathBuf::from(&args[i]));
            }
            _ if arg.starts_with("--config=") => {
                let path = arg.strip_prefix("--config=").unwrap();
                config = Some(PathBuf::from(path));
            }
            _ if !arg.starts_with('-') => {
                config = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("error: unrecognized option '{arg}'");
                eprintln!("Run 'harbor --help' for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    match config {
        Some(p) => (p, false),
        None => (default_config_path(), true),
    }
}

fn default_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("harbor/config.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/harbor/config.toml")
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join(".config/harbor/config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}

fn print_help() {
    println!(
        "harbor {} - zero-async file organizer daemon\n\n\
        USAGE:\n    \
            harbor [OPTIONS]\n    \
            harbor mime <FILE>...\n\n\
        COMMANDS:\n    \
            mime <FILE>...       Inspect detected MIME type of files\n\n\
        OPTIONS:\n    \
            -c, --config <PATH>  Path to configuration file\n    \
            -h, --help           Print help information\n    \
            -v, --version        Print version information",
        env!("CARGO_PKG_VERSION")
    );
}
