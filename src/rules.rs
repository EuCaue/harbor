use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};

use crate::config::{Mode, Folder};

#[derive(Clone)]
pub struct CompiledRule {
    matcher: GlobMatcher,
    name: String,
    dest: PathBuf,
    mode: Mode,
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
            .filter_map(|r| {
                glob(&r.match_pattern).map(|matcher| CompiledRule {
                    matcher,
                    name: r.name.clone(),
                    dest: r.to.clone(),
                    mode: r.mode,
                })
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
    pub fn find(&self, name: &str) -> Option<&CompiledRule> {
        self.rules.iter().find(|r| r.matcher.is_match(name))
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
        let r = wr.find("photo.jpg").unwrap();
        assert_eq!(r.to(), Path::new("/pics"));
        assert_eq!(r.mode(), Mode::Move);
        assert_eq!(r.name(), "Fotos");

        let r = wr.find("invoice-9.txt").unwrap();
        assert_eq!(r.to(), Path::new("/invoices"));
        assert_eq!(r.mode(), Mode::Copy);
    }

    #[test]
    fn fallback_star() {
        let wr = FolderRules::build(&folder());
        let r = wr.find("random.pdf").unwrap();
        assert_eq!(r.to(), Path::new("/misc"));
    }

    #[test]
    fn ignored_matches_global_and_local() {
        let wr = FolderRules::build(&folder());
        assert!(wr.ignored("x.part")); // global
        assert!(wr.ignored(".hidden")); // local
        assert!(!wr.ignored("photo.jpg"));
    }
}
