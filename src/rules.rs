use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use globset::{Glob, GlobMatcher};

use crate::config::{Folder, Mode};

#[derive(Clone)]
pub struct CompiledRule {
    matcher: Option<GlobMatcher>,
    name: String,
    dest: PathBuf,
    mode: Mode,
    mime_patterns: Vec<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
}

impl CompiledRule {
    pub fn resolve_dest(&self, file_path: &Path) -> PathBuf {
        let time = fs::metadata(file_path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());
        let dest_str = self.dest.to_string_lossy();
        if dest_str.contains('{') && dest_str.contains('}') {
            PathBuf::from(expand_date_placeholders(&dest_str, time))
        } else {
            self.dest.clone()
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone)]
pub struct FolderRules {
    rules: Vec<CompiledRule>,
    ignore: Vec<GlobMatcher>,
}

impl FolderRules {
    pub fn build(folder: &Folder) -> FolderRules {
        let rules = folder
            .rules
            .iter()
            .map(|r| {
                let matcher = if r.match_pattern.is_empty() {
                    None
                } else {
                    glob(&r.match_pattern)
                };
                CompiledRule {
                    matcher,
                    name: r.name.clone(),
                    dest: r.to.clone(),
                    mode: r.mode,
                    mime_patterns: r.mime_patterns.clone(),
                    min_size: r.min_size,
                    max_size: r.max_size,
                }
            })
            .collect();
        let ignore = folder
            .ignore_patterns
            .iter()
            .filter_map(|p| glob(p))
            .collect();
        FolderRules { rules, ignore }
    }

    pub fn ignored(&self, name: &str) -> bool {
        self.ignore.iter().any(|g| g.is_match(name))
    }

    /// First matching rule wins (config order = precedence).
    /// Evaluates: size filter -> glob match -> MIME match.
    pub fn find(&self, name: &str, file_path: &Path) -> Option<&CompiledRule> {
        self.rules.iter().find(|r| {
            if r.min_size.is_some() || r.max_size.is_some() {
                let size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
                if let Some(min) = r.min_size
                    && size < min
                {
                    return false;
                }
                if let Some(max) = r.max_size
                    && size > max
                {
                    return false;
                }
            }

            if r.matcher.as_ref().is_some_and(|m| m.is_match(name)) {
                return true;
            }
            if !r.mime_patterns.is_empty()
                && let Some(detected) = crate::mime::detect(file_path)
            {
                return r
                    .mime_patterns
                    .iter()
                    .any(|p| crate::mime::matches_mime_pattern(&detected, p));
            }
            false
        })
    }
}

pub fn expand_date_placeholders(s: &str, time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = (secs / 86400) as i64;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1461 - doe / 142401) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let year = format!("{y:04}");
    let month = format!("{m:02}");
    let day = format!("{d:02}");
    let date = format!("{year}-{month}-{day}");

    s.replace("{year}", &year)
        .replace("{YYYY}", &year)
        .replace("{month}", &month)
        .replace("{MM}", &month)
        .replace("{day}", &day)
        .replace("{DD}", &day)
        .replace("{date}", &date)
        .replace("{YYYY-MM-DD}", &date)
}

fn glob(p: &str) -> Option<GlobMatcher> {
    Glob::new(p).ok().map(|g| g.compile_matcher())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{parse, Folder};

    fn folder() -> Folder {
        parse(
            r#"
            [ignore]
            match = ["*.part"]

            [[folder]]
            path = "/x"

              [folder.options]
              wait = 0

              [folder.ignore]
              match = [".*"]

              [[folder.rule]]
              name = "Fotos"
              match = "*.{jpg,png}"
              to = "/pics"

              [[folder.rule]]
              match = "invoice-*"
              to = "/invoices"
              mode = "copy"

              [[folder.rule]]
              match = "*"
              to = "/misc"
        "#,
        )
        .unwrap()
        .folders
        .remove(0)
    }

    #[test]
    fn first_match_wins() {
        let wr = FolderRules::build(&folder());
        let r = wr.find("photo.jpg", Path::new("/x/photo.jpg")).unwrap();
        assert_eq!(r.resolve_dest(Path::new("/x/photo.jpg")), Path::new("/pics"));
        assert_eq!(r.mode(), Mode::Move);
        assert_eq!(r.name(), "Fotos");

        let r = wr.find("invoice-9.txt", Path::new("/x/invoice-9.txt")).unwrap();
        assert_eq!(r.resolve_dest(Path::new("/x/invoice-9.txt")), Path::new("/invoices"));
        assert_eq!(r.mode(), Mode::Copy);
    }

    #[test]
    fn fallback_star() {
        let wr = FolderRules::build(&folder());
        let r = wr.find("random.pdf", Path::new("/x/random.pdf")).unwrap();
        assert_eq!(r.resolve_dest(Path::new("/x/random.pdf")), Path::new("/misc"));
    }

    #[test]
    fn ignored_matches_global_and_local() {
        let wr = FolderRules::build(&folder());
        assert!(wr.ignored("x.part")); // global
        assert!(wr.ignored(".hidden")); // local
        assert!(!wr.ignored("photo.jpg"));
    }

    #[test]
    fn mime_rule_matches() {
        let cfg = parse(
            r#"
            [[folder]]
            path = "/x"

              [[folder.rule]]
              name = "Images"
              mime = "image/*"
              to = "/images"

              [[folder.rule]]
              name = "Docs"
              mime = ["application/pdf", "text/*"]
              to = "/docs"
        "#,
        )
        .unwrap();
        let wr = FolderRules::build(&cfg.folders[0]);

        // Extension fallback in mime::detect
        let r = wr.find("image_no_ext", Path::new("/x/photo.png")).unwrap();
        assert_eq!(r.name(), "Images");
        assert_eq!(r.resolve_dest(Path::new("/x/photo.png")), Path::new("/images"));

        let r = wr.find("doc.pdf", Path::new("/x/doc.pdf")).unwrap();
        assert_eq!(r.name(), "Docs");
        assert_eq!(r.resolve_dest(Path::new("/x/doc.pdf")), Path::new("/docs"));
    }

    #[test]
    fn date_placeholders_expansion() {
        // 2026-08-21T00:00:00Z = 1787270400 secs
        let fixed_time = UNIX_EPOCH + std::time::Duration::from_secs(1787270400);
        let s = "/photos/{year}/{month}/{day}/{date}";
        let res = expand_date_placeholders(s, fixed_time);
        assert_eq!(res, "/photos/2026/08/21/2026-08-21");
    }

    #[test]
    fn size_filter_matches() {
        let dir = std::env::temp_dir().join(format!("harbor_size_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let small = dir.join("small.txt");
        let big = dir.join("big.bin");
        fs::write(&small, b"hello").unwrap(); // 5 bytes
        fs::write(&big, vec![0u8; 2000]).unwrap(); // 2000 bytes

        let cfg = parse(
            r#"
            [[folder]]
            path = "/x"

              [[folder.rule]]
              name = "Big"
              match = "*"
              min_size = "1KB"
              to = "/big"

              [[folder.rule]]
              name = "Small"
              match = "*"
              max_size = "500B"
              to = "/small"
        "#,
        )
        .unwrap();
        let wr = FolderRules::build(&cfg.folders[0]);

        let r_small = wr.find("small.txt", &small).unwrap();
        assert_eq!(r_small.name(), "Small");

        let r_big = wr.find("big.bin", &big).unwrap();
        assert_eq!(r_big.name(), "Big");

        let _ = fs::remove_dir_all(dir);
    }
}
