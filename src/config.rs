use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Move,
    Copy,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub watches: Vec<Watch>,
}

#[derive(Debug, Clone)]
pub struct Watch {
    pub dir: PathBuf,
    pub defaults: Defaults,
    pub ignore_patterns: Vec<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct Defaults {
    pub cooldown_secs: u64,
    pub dedup: bool,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern: String,
    pub name: String,
    pub dest: PathBuf,
    pub mode: Mode,
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Config, String> {
    let raw: RawConfig = toml::from_str(text).map_err(|e| format!("toml: {}", e))?;
    resolve(raw)
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    defaults: RawDefaults,
    #[serde(default)]
    ignore: RawIgnore,
    watch: Vec<RawWatch>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct RawDefaults {
    dest: Option<PathBuf>,
    mode: Option<Mode>,
    cooldown_secs: Option<u64>,
    dedup: Option<bool>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct RawIgnore {
    patterns: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RawWatch {
    dir: PathBuf,
    #[serde(default)]
    defaults: RawDefaults,
    #[serde(default)]
    ignore: RawIgnore,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RawRule {
    pattern: String,
    dest: PathBuf,
    #[serde(default)]
    mode: Option<Mode>,
    #[serde(default)]
    name: Option<String>,
}

fn resolve(raw: RawConfig) -> Result<Config, String> {
    if raw.watch.is_empty() {
        return Err("no [[watch]] dirs defined".into());
    }

    let g = raw.defaults;
    let g_ignore = raw.ignore.patterns;

    let watches = raw
        .watch
        .into_iter()
        .map(|w| {
            let w_dir = expand(&w.dir);
            let eff_dest = expand(
                &w.defaults
                    .dest
                    .or_else(|| g.dest.clone())
                    .unwrap_or_else(|| w_dir.join("organized")),
            );
            let eff_mode = w.defaults.mode.or(g.mode).unwrap_or_default();
            let eff_cooldown = w.defaults.cooldown_secs.or(g.cooldown_secs).unwrap_or(5);
            let eff_dedup = w.defaults.dedup.or(g.dedup).unwrap_or(true);

            let mut rules: Vec<Rule> = w
                .rules
                .into_iter()
                .map(|r| Rule {
                    pattern: r.pattern.clone(),
                    name: r.name.unwrap_or(r.pattern),
                    dest: expand(&r.dest),
                    mode: r.mode.unwrap_or(eff_mode),
                })
                .collect();
            if rules.is_empty() {
                rules.push(Rule {
                    pattern: "*".into(),
                    name: "fallback".into(),
                    dest: eff_dest.clone(),
                    mode: eff_mode,
                });
            }

            let mut ignore_patterns = g_ignore.clone();
            ignore_patterns.extend(w.ignore.patterns);

            Watch {
                dir: w_dir,
                defaults: Defaults {
                    cooldown_secs: eff_cooldown,
                    dedup: eff_dedup,
                },
                ignore_patterns,
                rules,
            }
        })
        .collect();

    Ok(Config { watches })
}

/// Expands `$VAR` and `${VAR}` in paths. Unknown vars stay literal.
fn expand_env(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            if chars[i + 1] == '{' {
                if let Some(end_rel) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let name: String = chars[i + 2..i + 2 + end_rel].iter().collect();
                    match std::env::var(&name) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => out.push_str(&s[i..i + 3 + end_rel]),
                    }
                    i += 3 + end_rel;
                    continue;
                }
            } else if chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_' {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let name: String = chars[i + 1..j].iter().collect();
                match std::env::var(&name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('$');
                        out.push_str(&name);
                    }
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn expand(p: &Path) -> PathBuf {
    PathBuf::from(expand_env(&p.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Mode;

    const SAMPLE: &str = r#"
        [defaults]
        dest = "/home/me/Downloads/organized"
        mode = "move"
        cooldown_secs = 5
        dedup = true

        [ignore]
        patterns = ["*.part", "*.crdownload"]

        [[watch]]
        dir = "/home/me/Downloads"

          [watch.defaults]
          cooldown_secs = 15

          [watch.ignore]
          patterns = ["*.srt"]

          [[watch.rules]]
          pattern = "*.{jpg,png}"
          dest = "/home/me/Pictures"

          [[watch.rules]]
          pattern = "invoice-*"
          dest = "/home/me/work/invoices"
          mode = "copy"

        [[watch]]
        dir = "/mnt/usb/phone"

          [watch.defaults]
          cooldown_secs = 0
    "#;

    #[test]
    fn parses_and_merges() {
        let cfg = parse(SAMPLE).unwrap();
        assert_eq!(cfg.watches.len(), 2);

        let dl = &cfg.watches[0];
        assert_eq!(dl.dir, PathBuf::from("/home/me/Downloads"));
        assert_eq!(dl.rules[0].dest, PathBuf::from("/home/me/Pictures"));
        assert_eq!(dl.defaults.cooldown_secs, 15); // watch override
        assert_eq!(dl.rules[0].mode, Mode::Move); // inherited
        assert!(dl.defaults.dedup); // inherited
        assert_eq!(dl.ignore_patterns, vec!["*.part", "*.crdownload", "*.srt"]); // merged
        assert_eq!(dl.rules.len(), 2);
        assert_eq!(dl.rules[1].mode, Mode::Copy); // rule override
        assert_eq!(dl.rules[0].mode, Mode::Move); // rule inherits watch mode

        let usb = &cfg.watches[1];
        assert_eq!(usb.defaults.cooldown_secs, 0);
        assert_eq!(usb.rules.len(), 1); // implicit catch-all
        assert_eq!(usb.rules[0].pattern, "*");
        assert_eq!(usb.rules[0].name, "fallback");
        // global dest wins when set (watch-dir/organized only when no dest anywhere)
        assert_eq!(
            usb.rules[0].dest,
            PathBuf::from("/home/me/Downloads/organized")
        );
    }

    #[test]
    fn rule_name_defaults_to_pattern() {
        let cfg = parse(
            r#"
            [[watch]]
            dir = "/x"

              [[watch.rules]]
              name = "Fotos"
              pattern = "*.jpg"
              dest = "/pics"

              [[watch.rules]]
              pattern = "*.pdf"
              dest = "/docs"
        "#,
        )
        .unwrap();
        let w = &cfg.watches[0];
        assert_eq!(w.rules[0].name, "Fotos"); // explicit
        assert_eq!(w.rules[1].name, "*.pdf"); // defaults to pattern
    }

    #[test]
    fn expands_env_in_paths() {
        let home = std::env::var("HOME").unwrap();
        let cfg = parse(
            r#"
            [[watch]]
            dir = "$HOME/Downloads"

              [watch.defaults]
              dest = "${HOME}/org"

              [[watch.rules]]
              pattern = "*"
              dest = "$HOME/pics"

            [[watch]]
            dir = "$HOME/empty"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.watches[0].dir, PathBuf::from(&home).join("Downloads"));
        assert_eq!(
            cfg.watches[0].rules[0].dest,
            PathBuf::from(&home).join("pics")
        );
        // watch without rules and no default dest: implicit fallback = <dir>/organized
        assert_eq!(
            cfg.watches[1].rules[0].dest,
            PathBuf::from(&home).join("empty/organized")
        );
    }

    #[test]
    fn unknown_env_stays_literal() {
        let cfg = parse(
            r#"
            [[watch]]
            dir = "/x"

              [watch.defaults]
              dest = "$HARBOR_DEFINITELY_UNSET/dst"
        "#,
        )
        .unwrap();
        assert_eq!(
            cfg.watches[0].rules[0].dest,
            PathBuf::from("$HARBOR_DEFINITELY_UNSET/dst")
        );
    }

    #[test]
    fn empty_watch_rejected() {
        assert!(parse("[watch]").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn unknown_key_rejected() {
        let bad = "[[watch]]\ndir = \"/x\"\nbanana = true\n";
        assert!(parse(bad).is_err());
    }
}
