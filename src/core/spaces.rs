//! Spaces: named note folders, like Obsidian vaults. The list lives in
//! `$XDG_CONFIG_HOME/abstract/spaces` as one absolute path per line; the line
//! prefixed with `* ` is the active space.

use std::path::{Path, PathBuf};

use crate::core::store::{write_atomic, xdg};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spaces {
    pub paths: Vec<PathBuf>,
    pub active: usize,
}

impl Spaces {
    pub fn parse(src: &str) -> Self {
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut active = 0;
        for line in src.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let (is_active, path) = match line.strip_prefix("* ") {
                Some(p) => (true, p.trim()),
                None => (false, line),
            };
            let path = PathBuf::from(path);
            if paths.contains(&path) {
                continue;
            }
            if is_active {
                active = paths.len();
            }
            paths.push(path);
        }
        Self { paths, active }
    }

    pub fn serialize(&self) -> String {
        self.paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}{}\n",
                    if i == self.active { "* " } else { "" },
                    p.display()
                )
            })
            .collect()
    }

    pub fn current(&self) -> &Path {
        &self.paths[self.active]
    }

    /// Adds `path` if new and makes it active.
    pub fn activate_path(&mut self, path: PathBuf) {
        self.active = match self.paths.iter().position(|p| *p == path) {
            Some(i) => i,
            None => {
                self.paths.push(path);
                self.paths.len() - 1
            }
        };
    }

    /// Forgets a space (files are untouched). The last space cannot be removed.
    pub fn remove(&mut self, ix: usize) -> bool {
        if self.paths.len() <= 1 || ix >= self.paths.len() {
            return false;
        }
        self.paths.remove(ix);
        if self.active > ix || self.active >= self.paths.len() {
            self.active = self.active.saturating_sub(1);
        }
        true
    }
}

pub fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn config_file() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
        .join("abstract")
        .join("spaces")
}

/// Blocking. Loads the list, seeding `~/.local/share/abstract/Pessoal` on first
/// run. A CLI path argument is added and activated.
pub fn load(cli_path: Option<PathBuf>) -> Spaces {
    let mut spaces = std::fs::read_to_string(config_file())
        .map(|s| Spaces::parse(&s))
        .unwrap_or(Spaces {
            paths: Vec::new(),
            active: 0,
        });
    if spaces.paths.is_empty() {
        let path = xdg("XDG_DATA_HOME", ".local/share")
            .join("abstract")
            .join("Pessoal");
        let _ = std::fs::create_dir_all(&path);
        spaces.paths.push(path);
    }
    if let Some(arg) = cli_path {
        let p = arg;
        spaces.activate_path(std::fs::canonicalize(&p).unwrap_or(p));
    }
    spaces
}

/// Blocking.
pub fn save(spaces: &Spaces) {
    let file = config_file();
    let result = write_atomic(&file, spaces.serialize().as_bytes());
    if let Err(err) = result {
        eprintln!(
            "abstract: failed to save spaces to {}: {err}",
            file.display()
        );
    }
}

#[cfg(test)]
mod tests;
