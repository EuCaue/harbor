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
    pub folders: Vec<Folder>,
}

#[derive(Debug, Clone)]
pub struct Folder {
    pub path: PathBuf,
    pub options: Options,
    pub ignore_patterns: Vec<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub wait: u64,
    pub overwrite: bool,
    pub include_dirs: bool,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub match_pattern: String,
    pub name: String,
    pub to: PathBuf,
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
    defaults: RawOptions,
    #[serde(default)]
    ignore: RawIgnore,
    #[serde(rename = "folder", default)]
    folders: Vec<RawFolder>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct RawOptions {
    #[serde(rename = "to")]
    to: Option<PathBuf>,
    mode: Option<Mode>,
    #[serde(rename = "wait")]
    wait: Option<u64>,
    #[serde(rename = "overwrite")]
    overwrite: Option<bool>,
    #[serde(rename = "include_dirs")]
    include_dirs: Option<bool>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct RawIgnore {
    #[serde(rename = "match", default)]
    matches: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RawFolder {
    path: PathBuf,
    #[serde(rename = "options", default)]
    options: RawOptions,
    #[serde(default)]
    ignore: RawIgnore,
    #[serde(rename = "rule", default)]
    rules: Vec<RawRule>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RawRule {
    #[serde(rename = "match")]
    match_pattern: String,
    #[serde(rename = "to")]
    to: PathBuf,
    #[serde(default)]
    mode: Option<Mode>,
    #[serde(default)]
    name: Option<String>,
}

fn resolve(raw: RawConfig) -> Result<Config, String> {
    if raw.folders.is_empty() {
        return Err("no [[folder]] dirs defined".into());
    }

    let g = raw.defaults;
    let g_ignore = raw.ignore.matches;

    let folders = raw
        .folders
        .into_iter()
        .map(|f| {
            let f_path = expand(&f.path);
            let eff_to = expand(
                &f.options
                    .to
                    .or_else(|| g.to.clone())
                    .unwrap_or_else(|| f_path.join("organized")),
            );
            let eff_mode = f.options.mode.or(g.mode).unwrap_or_default();
            let eff_wait = f.options.wait.or(g.wait).unwrap_or(5);
            let eff_overwrite = f.options.overwrite.or(g.overwrite).unwrap_or(false);
            let eff_include_dirs = f.options.include_dirs.or(g.include_dirs).unwrap_or(false);

            let mut rules: Vec<Rule> = f
                .rules
                .into_iter()
                .map(|r| Rule {
                    match_pattern: r.match_pattern.clone(),
                    name: r.name.unwrap_or(r.match_pattern),
                    to: expand(&r.to),
                    mode: r.mode.unwrap_or(eff_mode),
                })
                .collect();
            if rules.is_empty() {
                rules.push(Rule {
                    match_pattern: "*".into(),
                    name: "fallback".into(),
                    to: eff_to.clone(),
                    mode: eff_mode,
                });
            }

            let mut ignore_patterns = g_ignore.clone();
            ignore_patterns.extend(f.ignore.matches);

            Folder {
                path: f_path,
                options: Options {
                    wait: eff_wait,
                    overwrite: eff_overwrite,
                    include_dirs: eff_include_dirs,
                },
                ignore_patterns,
                rules,
            }
        })
        .collect();

    Ok(Config { folders })
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
        to = "/home/me/Downloads/organized"
        mode = "move"
        wait = 5
        overwrite = false

        [ignore]
        match = ["*.part", "*.crdownload"]

        [[folder]]
        path = "/home/me/Downloads"

          [folder.options]
          wait = 15

          [folder.ignore]
          match = ["*.srt"]

          [[folder.rule]]
          match = "*.{jpg,png}"
          to = "/home/me/Pictures"

          [[folder.rule]]
          match = "invoice-*"
          to = "/home/me/work/invoices"
          mode = "copy"

        [[folder]]
        path = "/mnt/usb/phone"

          [folder.options]
          wait = 0
    "#;

    #[test]
    fn parses_and_merges() {
        let cfg = parse(SAMPLE).unwrap();
        assert_eq!(cfg.folders.len(), 2);

        let dl = &cfg.folders[0];
        assert_eq!(dl.path, PathBuf::from("/home/me/Downloads"));
        assert_eq!(dl.rules[0].to, PathBuf::from("/home/me/Pictures"));
        assert_eq!(dl.options.wait, 15); // folder override
        assert_eq!(dl.rules[0].mode, Mode::Move); // inherited
        assert!(!dl.options.overwrite); // inherited
        assert_eq!(dl.ignore_patterns, vec!["*.part", "*.crdownload", "*.srt"]); // merged
        assert_eq!(dl.rules.len(), 2);
        assert_eq!(dl.rules[1].mode, Mode::Copy); // rule override
        assert_eq!(dl.rules[0].mode, Mode::Move); // rule inherits folder mode

        let usb = &cfg.folders[1];
        assert_eq!(usb.options.wait, 0);
        assert_eq!(usb.rules.len(), 1); // implicit catch-all
        assert_eq!(usb.rules[0].match_pattern, "*");
        assert_eq!(usb.rules[0].name, "fallback");
        // global to wins when set (folder-path/organized only when no to anywhere)
        assert_eq!(
            usb.rules[0].to,
            PathBuf::from("/home/me/Downloads/organized")
        );
    }

    #[test]
    fn rule_name_defaults_to_pattern() {
        let cfg = parse(
            r#"
            [[folder]]
            path = "/x"

              [[folder.rule]]
              name = "Fotos"
              match = "*.jpg"
              to = "/pics"

              [[folder.rule]]
              match = "*.pdf"
              to = "/docs"
        "#,
        )
        .unwrap();
        let w = &cfg.folders[0];
        assert_eq!(w.rules[0].name, "Fotos"); // explicit
        assert_eq!(w.rules[1].name, "*.pdf"); // defaults to pattern
    }

    #[test]
    fn expands_env_in_paths() {
        let home = std::env::var("HOME").unwrap();
        let cfg = parse(
            r#"
            [[folder]]
            path = "$HOME/Downloads"

              [folder.options]
              to = "${HOME}/org"

              [[folder.rule]]
              match = "*"
              to = "$HOME/pics"

            [[folder]]
            path = "$HOME/empty"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.folders[0].path, PathBuf::from(&home).join("Downloads"));
        assert_eq!(
            cfg.folders[0].rules[0].to,
            PathBuf::from(&home).join("pics")
        );
        assert_eq!(
            cfg.folders[1].rules[0].to,
            PathBuf::from(&home).join("empty/organized")
        );
    }

    #[test]
    fn unknown_env_stays_literal() {
        let cfg = parse(
            r#"
            [[folder]]
            path = "/x"

              [folder.options]
              to = "$HARBOR_DEFINITELY_UNSET/dst"
        "#,
        )
        .unwrap();
        assert_eq!(
            cfg.folders[0].rules[0].to,
            PathBuf::from("$HARBOR_DEFINITELY_UNSET/dst")
        );
    }

    #[test]
    fn empty_folder_rejected() {
        assert!(parse("[folder]").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn parses_include_dirs_option() {
        let cfg = parse(
            r#"
            [defaults]
            include_dirs = true

            [[folder]]
            path = "/x"
        "#,
        )
        .unwrap();
        assert!(cfg.folders[0].options.include_dirs);
    }
}
