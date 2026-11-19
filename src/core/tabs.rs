use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Tabs {
    paths: Vec<PathBuf>,
    active: Option<usize>,
}

impl Tabs {
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn active(&self) -> Option<&Path> {
        self.active
            .and_then(|i| self.paths.get(i))
            .map(PathBuf::as_path)
    }

    pub fn insert(&mut self, path: PathBuf) -> usize {
        if let Some(i) = self.paths.iter().position(|p| p == &path) {
            return i;
        }
        self.paths.push(path);
        self.paths.len() - 1
    }

    pub fn open(&mut self, path: PathBuf) {
        let i = self.insert(path);
        self.select(i);
    }

    pub fn select(&mut self, i: usize) {
        if i < self.paths.len() {
            self.active = Some(i);
        }
    }

    pub fn digit(&mut self, digit: usize) {
        if digit == 9 {
            self.select(self.paths.len().saturating_sub(1));
        } else if digit > 0 && digit < 9 {
            self.select(digit - 1);
        }
    }

    pub fn cycle(&mut self, forward: bool) {
        if self.paths.is_empty() {
            return;
        }
        let i = self.active.unwrap_or(0);
        self.select(if forward {
            (i + 1) % self.paths.len()
        } else {
            (i + self.paths.len() - 1) % self.paths.len()
        });
    }

    pub fn close(&mut self, path: &Path) {
        let Some(i) = self.paths.iter().position(|p| p == path) else {
            return;
        };
        self.paths.remove(i);
        self.active = self.active.and_then(|a| {
            if self.paths.is_empty() {
                None
            } else if a > i {
                Some(a - 1)
            } else {
                Some(a.min(self.paths.len() - 1))
            }
        });
    }

    pub fn relocate(&mut self, from: &Path, to: &Path) {
        for path in &mut self.paths {
            *path = crate::core::vault::relocated(path, from, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_focus_cycle_and_digits() {
        let mut tabs = Tabs::default();
        for p in ["a", "b", "c", "a"] {
            tabs.open(p.into());
        }
        assert_eq!(tabs.paths(), &[PathBuf::from("a"), "b".into(), "c".into()]);
        tabs.cycle(false);
        assert_eq!(tabs.active(), Some(Path::new("c")));
        tabs.cycle(true);
        assert_eq!(tabs.active(), Some(Path::new("a")));
        tabs.digit(9);
        assert_eq!(tabs.active(), Some(Path::new("c")));
        tabs.digit(2);
        tabs.close(Path::new("a"));
        assert_eq!(tabs.active(), Some(Path::new("b")));
        tabs.close(Path::new("b"));
        assert_eq!(tabs.active(), Some(Path::new("c")));
        tabs.close(Path::new("c"));
        assert_eq!(tabs.active(), None);
    }

    #[test]
    fn empty_and_invalid_actions_are_noops() {
        let mut tabs = Tabs::default();
        tabs.cycle(true);
        tabs.cycle(false);
        tabs.digit(9);
        tabs.close(Path::new("absent"));
        assert_eq!(tabs.active(), None);
        tabs.open("café".into());
        tabs.select(20);
        tabs.digit(0);
        tabs.digit(8);
        tabs.close(Path::new("absent"));
        assert_eq!(tabs.active(), Some(Path::new("café")));
    }
}
