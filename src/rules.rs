use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};

use crate::config::{Mode, Folder};

#[derive(Clone)]
pub struct CompiledRule {
    matcher: Option<GlobMatcher>,
    name: String,
    dest: PathBuf,
    mode: Mode,
    mime_patterns: Vec<String>,
}

impl CompiledRule {
    pub fn to(&self) -> &Path {
        &self.dest
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
    /// Within a rule: glob match takes priority over MIME match.
    pub fn find(&self, name: &str, file_path: &Path) -> Option<&CompiledRule> {
        self.rules.iter().find(|r| {
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
        assert_eq!(r.to(), Path::new("/pics"));
        assert_eq!(r.mode(), Mode::Move);
        assert_eq!(r.name(), "Fotos");

        let r = wr.find("invoice-9.txt", Path::new("/x/invoice-9.txt")).unwrap();
        assert_eq!(r.to(), Path::new("/invoices"));
        assert_eq!(r.mode(), Mode::Copy);
    }

    #[test]
    fn fallback_star() {
        let wr = FolderRules::build(&folder());
        let r = wr.find("random.pdf", Path::new("/x/random.pdf")).unwrap();
        assert_eq!(r.to(), Path::new("/misc"));
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
        assert_eq!(r.to(), Path::new("/images"));

        let r = wr.find("doc.pdf", Path::new("/x/doc.pdf")).unwrap();
        assert_eq!(r.name(), "Docs");
        assert_eq!(r.to(), Path::new("/docs"));
    }
}
