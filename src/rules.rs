use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};

use crate::config::{Mode, Watch};

#[derive(Clone)]
pub struct CompiledRule {
    matcher: GlobMatcher,
    name: String,
    dest: PathBuf,
    mode: Mode,
}

impl CompiledRule {
    pub fn dest(&self) -> &Path {
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
pub struct WatchRules {
    rules: Vec<CompiledRule>,
    ignore: Vec<GlobMatcher>,
}

impl WatchRules {
    pub fn build(watch: &Watch) -> WatchRules {
        let rules = watch
            .rules
            .iter()
            .filter_map(|r| {
                glob(&r.pattern).map(|matcher| CompiledRule {
                    matcher,
                    name: r.name.clone(),
                    dest: r.dest.clone(),
                    mode: r.mode,
                })
            })
            .collect();
        let ignore = watch
            .ignore_patterns
            .iter()
            .filter_map(|p| glob(p))
            .collect();
        WatchRules { rules, ignore }
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
    use crate::config::{parse, Watch};

    fn watch() -> Watch {
        parse(
            r#"
            [ignore]
            patterns = ["*.part"]

            [[watch]]
            dir = "/x"

              [watch.defaults]
              cooldown_secs = 0

              [watch.ignore]
              patterns = [".*"]

              [[watch.rules]]
              name = "Fotos"
              pattern = "*.{jpg,png}"
              dest = "/pics"

              [[watch.rules]]
              pattern = "invoice-*"
              dest = "/invoices"
              mode = "copy"

              [[watch.rules]]
              pattern = "*"
              dest = "/misc"
        "#,
        )
        .unwrap()
        .watches
        .remove(0)
    }

    #[test]
    fn first_match_wins() {
        let wr = WatchRules::build(&watch());
        let r = wr.find("photo.jpg").unwrap();
        assert_eq!(r.dest(), Path::new("/pics"));
        assert_eq!(r.mode(), Mode::Move);
        assert_eq!(r.name(), "Fotos");

        let r = wr.find("invoice-9.txt").unwrap();
        assert_eq!(r.dest(), Path::new("/invoices"));
        assert_eq!(r.mode(), Mode::Copy);
    }

    #[test]
    fn fallback_star() {
        let wr = WatchRules::build(&watch());
        let r = wr.find("random.pdf").unwrap();
        assert_eq!(r.dest(), Path::new("/misc"));
    }

    #[test]
    fn ignored_matches_global_and_local() {
        let wr = WatchRules::build(&watch());
        assert!(wr.ignored("x.part")); // global
        assert!(wr.ignored(".hidden")); // local
        assert!(!wr.ignored("photo.jpg"));
    }
}
