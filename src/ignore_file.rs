use std::env::current_dir;
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yml::from_reader;

#[derive(Deserialize, Default)]
pub struct Ignore {
    ignored_paths: Vec<PathBuf>,
}

impl Ignore {
    pub fn from_file<P: AsRef<Path>>(file: P) -> anyhow::Result<Self> {
        let file = File::open(file)?;
        let ignore = from_reader(file)?;
        Ok(ignore)
    }

    pub fn exclude<P: AsRef<Path>>(&self, path: P) -> bool {
        for ignored_path in &self.ignored_paths {
            if ignored_path == path.as_ref() {
                return true;
            }
        }
        false
    }
}

pub fn find_ignore_file() -> Option<PathBuf> {
    let mut path = current_dir().ok()?;

    loop {
        let ignore = path.join(".link_checker.yml");
        if ignore.exists() {
            return Some(ignore);
        }
        path = path.parent()?.to_owned();
    }
}
