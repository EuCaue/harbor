mod config;
mod daemon;
mod files;
mod mime;
mod rules;

use std::path::PathBuf;

enum Command {
    Run { config: PathBuf, is_default: bool },
    DryRun { config: PathBuf },
    Check { config: PathBuf },
    Mime { files: Vec<PathBuf> },
    Log { lines: usize, clear: bool },
}

fn main() {
    match parse_args() {
        Command::Check { config } => {
            if let Err(e) = config::check(&config) {
                eprintln!("harbor: {e}");
                std::process::exit(1);
            }
        }
        Command::DryRun { config } => {
            let cfg = config::load(&config).unwrap_or_else(|e| {
                eprintln!("harbor: {e}");
                std::process::exit(1);
            });
            daemon::dry_run(&cfg.folders);
        }
        Command::Log { lines, clear } => {
            if let Err(e) = daemon::show_log(lines, clear) {
                eprintln!("harbor: {e}");
                std::process::exit(1);
            }
        }
        Command::Mime { files } => {
            for file in files {
                match mime::detect(&file) {
                    Some(m) => println!("{}: {m}", file.display()),
                    None => println!("{}: unknown", file.display()),
                }
            }
        }
        Command::Run { config, is_default } => {
            if is_default && !config.exists() {
                if let Some(parent) = config.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&config, config::DEFAULT_CONFIG_TEMPLATE).is_ok() {
                    println!("harbor: created initial config at {}", config.display());
                    println!("harbor: edit your rules and run harbor again to start.");
                    return;
                }
            }

            let cfg = config::load(&config).unwrap_or_else(|e| {
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
    }
}

fn parse_args() -> Command {
    let args: Vec<String> = std::env::args().collect();
    let mut config: Option<PathBuf> = None;
    let mut is_dry_run = false;
    let mut is_check = false;
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
            "check" => {
                is_check = true;
            }
            "log" => {
                let mut lines = 40;
                let mut clear = false;
                i += 1;
                while i < args.len() {
                    let log_arg = &args[i];
                    if log_arg == "-c" || log_arg == "--clear" {
                        clear = true;
                    } else if log_arg == "-n" || log_arg == "--lines" {
                        i += 1;
                        if i >= args.len() {
                            eprintln!("error: flag '{log_arg}' requires a number argument");
                            std::process::exit(1);
                        }
                        lines = args[i].parse().unwrap_or(40);
                    } else if let Some(n_str) = log_arg.strip_prefix("-n=") {
                        lines = n_str.parse().unwrap_or(40);
                    } else if let Some(n_str) = log_arg.strip_prefix("--lines=") {
                        lines = n_str.parse().unwrap_or(40);
                    }
                    i += 1;
                }
                return Command::Log { lines, clear };
            }
            "-n" | "--dry-run" => {
                is_dry_run = true;
            }
            "mime" => {
                if i + 1 >= args.len() {
                    eprintln!("usage: harbor mime <file>...");
                    std::process::exit(1);
                }
                let files = args[i + 1..].iter().map(PathBuf::from).collect();
                return Command::Mime { files };
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

    let is_default = config.is_none();
    let config_path = config.unwrap_or_else(default_config_path);

    if is_check {
        Command::Check { config: config_path }
    } else if is_dry_run {
        Command::DryRun { config: config_path }
    } else {
        Command::Run {
            config: config_path,
            is_default,
        }
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
            harbor check [OPTIONS]\n    \
            harbor log [OPTIONS]\n    \
            harbor mime <FILE>...\n\n\
        COMMANDS:\n    \
            check                Validate configuration file syntax and paths\n    \
            log                  View or clear background organization history\n    \
            mime <FILE>...       Inspect detected MIME type of files\n\n\
        OPTIONS:\n    \
            -c, --config <PATH>  Path to configuration file\n    \
            -n, --dry-run        Simulate moves without touching files on disk\n    \
            -h, --help           Print help information\n    \
            -v, --version        Print version information\n\n\
        LOG OPTIONS:\n    \
            -n, --lines <NUM>    Number of log lines to show (default: 40)\n    \
            -c, --clear          Clear log history file",
        env!("CARGO_PKG_VERSION")
    );
}
