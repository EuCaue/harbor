use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::Folder;
use crate::files;
use crate::rules::FolderRules;

enum Msg {
    File(PathBuf),
    Moved(Row),
}

struct Row {
    rule: String,
    file: String,
    dest: String,
}

pub fn start(folders: Vec<Folder>) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let mut watchers = Vec::new();
    for w in &folders {
        let tx = tx.clone();
        let include_dirs = w.options.include_dirs;
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
                let ev = match res {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("folder error: {e}");
                        return;
                    }
                };
                match ev.kind {
                    // mv/rename into the path = Modify(Name), not Create
                    EventKind::Create(_)
                    | EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Name(_)) => {
                        for p in ev.paths {
                            if p.is_file() || (include_dirs && p.is_dir()) {
                                let _ = tx.send(Msg::File(p));
                            }
                        }
                    }
                    _ => {}
                }
            })
        .map_err(|e| format!("watcher: {e}"))?;
        watcher
            .watch(&w.path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("folder {}: {}", w.path.display(), e))?;
        watchers.push(watcher);
    }
    let tx_run = tx.clone();
    drop(tx);

    thread::spawn(move || run(rx, folders, watchers, tx_run));
    Ok(())
}

const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const FLUSH_BATCH: usize = 50;

fn run(
    rx: mpsc::Receiver<Msg>,
    folders: Vec<Folder>,
    watchers: Vec<RecommendedWatcher>,
    tx: mpsc::Sender<Msg>,
) {
    let _watchers = watchers;

    let pending: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let compiled: Vec<FolderRules> = folders.iter().map(FolderRules::build).collect();
    let mut rows: Vec<Row> = Vec::new();

    loop {
        match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(Msg::File(p)) => {
                if is_protected_dir(&p, &folders) {
                    continue;
                }
                if !pending.lock().unwrap().insert(p.clone()) {
                    continue; // already being processed
                }
                let Some(idx) = folders.iter().position(|w| p.starts_with(&w.path)) else {
                    pending.lock().unwrap().remove(&p);
                    continue;
                };
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                    pending.lock().unwrap().remove(&p);
                    continue;
                };
                let name = name.to_owned();
                if compiled[idx].ignored(&name) {
                    pending.lock().unwrap().remove(&p);
                    continue;
                }

                let folder = folders[idx].clone();
                let wr = compiled[idx].clone();
                let pending2 = pending.clone();
                let tx2 = tx.clone();
                thread::spawn(move || {
                    process_file(p.clone(), folder, wr, tx2);
                    pending2.lock().unwrap().remove(&p);
                });
            }
            Ok(Msg::Moved(row)) => {
                rows.push(row);
                if rows.len() >= FLUSH_BATCH {
                    flush(&mut rows);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => flush(&mut rows),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                flush(&mut rows);
                break;
            }
        }
    }
}

fn process_file(p: PathBuf, folder: Folder, wr: FolderRules, tx: mpsc::Sender<Msg>) {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let cooldown = Duration::from_secs(folder.options.wait);
    if !wait_stable(&p, cooldown) {
        return; // file vanished or never stabilised
    }

    let Some(rule) = wr.find(&name) else {
        return;
    };

    match files::apply(&p, rule.to(), rule.mode(), !folder.options.overwrite) {
        Ok(dst) => {
            let row = Row {
                rule: rule.name().to_string(),
                file: name,
                dest: dst.to_string_lossy().to_string(),
            };
            let _ = tx.send(Msg::Moved(row));
        }
        Err(e) => eprintln!("{}: {e}", p.display()),
    }
}

fn flush(rows: &mut Vec<Row>) {
    if rows.is_empty() {
        return;
    }
    rows.sort_by(|a, b| a.rule.cmp(&b.rule).then(a.file.cmp(&b.file)));
    let n = rows.len();
    let cats = rows
        .iter()
        .map(|r| r.rule.as_str())
        .collect::<HashSet<_>>()
        .len();
    println!(
        "harbor: {n} file(s) organized in {cats} categories\n{}\n",
        render_table(rows)
    );
    rows.clear();
}

/// Real data table: auto-width columns, header, rows grouped by rule.
fn render_table(rows: &[Row]) -> String {
    const DEST_CAP: usize = 80;
    let hdr = ["RULE", "FILE", "DESTINATION"];
    let maxw = |c: usize, idx: usize| {
        c.max(
            rows.iter()
                .map(|r| match idx {
                    0 => r.rule.chars().count(),
                    1 => r.file.chars().count(),
                    _ => r.dest.chars().count().min(DEST_CAP),
                })
                .max()
                .unwrap_or(0),
        )
    };
    let nw = maxw(hdr[0].chars().count(), 0);
    let fw = maxw(hdr[1].chars().count(), 1);
    let dw = maxw(hdr[2].chars().count(), 2);

    let bar = |w: usize| "─".repeat(w + 2);
    let top = format!("┌{}┬{}┬{}┐", bar(nw), bar(fw), bar(dw));
    let sep = format!("├{}┼{}┼{}┤", bar(nw), bar(fw), bar(dw));
    let bot = format!("└{}┴{}┴{}┘", bar(nw), bar(fw), bar(dw));
    let line = |a: &str, b: &str, c: &str| {
        format!("│ {} │ {} │ {} │", fit(a, nw), fit(b, fw), fit_tail(c, dw))
    };

    let mut out = vec![top, line("RULE", "FILE", "DESTINATION"), sep.clone()];
    let mut prev: Option<&str> = None;
    for r in rows {
        if prev.is_some_and(|p| p != r.rule.as_str()) {
            out.push(sep.clone());
        }
        out.push(line(&r.rule, &r.file, &r.dest));
        prev = Some(&r.rule);
    }
    out.push(bot);
    out.join("\n")
}

/// Pads to `w` chars, truncates the head with `…` when over.
fn fit(s: &str, w: usize) -> String {
    let c = s.chars().count();
    if c <= w {
        let mut out = s.to_string();
        out.push_str(&" ".repeat(w - c));
        out
    } else {
        let mut out: String = s.chars().take(w - 1).collect();
        out.push('…');
        out
    }
}

/// Keeps the tail of long paths (the interesting part), truncating the head.
fn fit_tail(s: &str, w: usize) -> String {
    let c = s.chars().count();
    if c <= w {
        let mut out = s.to_string();
        out.push_str(&" ".repeat(w - c));
        out
    } else {
        let keep = w - 1;
        let tail: String = s.chars().skip(c - keep).collect();
        format!("…{tail}")
    }
}

fn is_protected_dir(p: &Path, folders: &[Folder]) -> bool {
    for f in folders {
        if p == f.path {
            return true;
        }
        for r in &f.rules {
            if p == r.to || p.starts_with(&r.to) || r.to.starts_with(p) {
                return true;
            }
        }
    }
    false
}

/// Returns true when the file or directory has been unchanged for `cooldown`.
/// Returns false if it disappears mid-wait.
fn wait_stable(p: &Path, cooldown: Duration) -> bool {
    if cooldown.is_zero() {
        return p.exists();
    }
    let mut last = entry_sig(p);
    let mut stable = Duration::ZERO;
    while let Some(sig) = last {
        thread::sleep(Duration::from_secs(1));
        let now = entry_sig(p);
        match now {
            None => return false,
            Some(s) if s == sig => {
                stable += Duration::from_secs(1);
                if stable >= cooldown {
                    return true;
                }
            }
            Some(s) => {
                stable = Duration::ZERO;
                last = Some(s);
            }
        }
    }
    false
}

fn entry_sig(p: &Path) -> Option<(u64, SystemTime)> {
    let m = fs::metadata(p).ok()?;
    Some((m.len(), m.modified().unwrap_or(UNIX_EPOCH)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;
    use std::time::Instant;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("harbor-daemon-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn start_with(src: &Path, dst: &Path) {
        let cfg = parse(&format!(
            r#"
            [[folder]]
            path = "{}"

              [folder.options]
              to = "{}"
              wait = 1
        "#,
            src.display(),
            dst.display()
        ))
        .unwrap();
        start(cfg.folders).unwrap()
    }

    fn wait_for(path: &Path, timeout_secs: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while Instant::now() < deadline {
            if path.exists() {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn created_file_is_moved() {
        let src = tmpdir("src");
        let dst = tmpdir("dst");
        start_with(&src, &dst);

        fs::write(src.join("movie.mp4"), b"data").unwrap();
        let moved = wait_for(&dst.join("movie.mp4"), 10);
        assert!(moved, "file never moved");
        assert_eq!(fs::read_to_string(dst.join("movie.mp4")).unwrap(), "data");
        assert!(!src.join("movie.mp4").exists());
    }

    #[test]
    fn renamed_in_file_is_moved() {
        let src = tmpdir("src2");
        let staging = tmpdir("staging");
        let dst = tmpdir("dst2");
        start_with(&src, &dst);

        // mv = rename(2) => Modify(Name), the case browsers use (partial -> final)
        fs::write(staging.join("thing.txt"), b"data").unwrap();
        fs::rename(staging.join("thing.txt"), src.join("thing.txt")).unwrap();
        let moved = wait_for(&dst.join("thing.txt"), 10);
        assert!(moved, "file never moved after rename into foldered dir");
        assert!(!src.join("thing.txt").exists());
    }

    #[test]
    fn table_groups_by_rule_and_aligns() {
        let rows = vec![
            Row {
                rule: "Musica".into(),
                file: "album.flac".into(),
                dest: "/o/Music/album.flac".into(),
            },
            Row {
                rule: "Imagens".into(),
                file: "férias_2025.jpg".into(),
                dest: "/o/Images/férias_2025.jpg".into(),
            },
            Row {
                rule: "Imagens".into(),
                file: "logo.svg".into(),
                dest: "/o/Images/logo.svg".into(),
            },
        ];
        let table = render_table(&rows);
        let lines: Vec<&str> = table.lines().collect();
        // top + header + sep + 2 groups (sep between) + 3 rows + bottom
        assert_eq!(lines.len(), 8);
        assert!(lines[0].starts_with('┌'));
        assert!(lines[1].starts_with('│'));
        assert!(lines[2].starts_with('├'));
        assert!(lines[lines.len() - 1].starts_with('└'));
        // every row line has exactly 4 vertical bars, multibyte-safe
        assert!(lines
            .iter()
            .filter(|l| l.starts_with('│'))
            .all(|l| l.matches('│').count() == 4));
    }

    #[test]
    fn protected_dirs_are_safely_ignored() {
        let f1 = Folder {
            path: PathBuf::from("/home/user/Downloads"),
            options: crate::config::Options {
                wait: 1,
                overwrite: false,
                include_dirs: true,
            },
            ignore_patterns: vec![],
            rules: vec![crate::config::Rule {
                match_pattern: "*".into(),
                name: "test".into(),
                to: PathBuf::from("/home/user/Downloads/Documents"),
                mode: crate::config::Mode::Move,
            }],
        };
        let folders = vec![f1];
        // Watched dir itself protected
        assert!(is_protected_dir(Path::new("/home/user/Downloads"), &folders));
        // Rule target dir protected
        assert!(is_protected_dir(Path::new("/home/user/Downloads/Documents"), &folders));
        // Child of rule target dir protected
        assert!(is_protected_dir(Path::new("/home/user/Downloads/Documents/sub"), &folders));
        // Normal downloaded folder NOT protected
        assert!(!is_protected_dir(Path::new("/home/user/Downloads/MyFolder"), &folders));
    }
}
