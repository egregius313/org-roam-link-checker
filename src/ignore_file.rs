use std::env::current_dir;
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yml::from_reader;

#[derive(Deserialize, Default)]
pub struct Ignore {
    #[serde(default)]
    ignored_paths: Vec<PathBuf>,
    #[serde(rename = "roam-db", default)]
    roam_db: Option<PathBuf>,
}

impl Ignore {
    pub fn from_file<P: AsRef<Path>>(file: P) -> anyhow::Result<Self> {
        let path = file.as_ref();
        let file = File::open(path)?;
        let mut ignore: Self = from_reader(file)?;
        if let Some(db_path) = ignore.roam_db.as_mut().filter(|path| path.is_relative()) {
            *db_path = path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(&*db_path);
        }
        Ok(ignore)
    }

    pub fn roam_db_path(&self) -> Option<&Path> {
        self.roam_db.as_deref()
    }

    pub fn add_paths<I: IntoIterator<Item = PathBuf>>(&mut self, paths: I) {
        self.ignored_paths.extend(paths);
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
        for name in [".link-checker.yml", ".link_checker.yml"] {
            let config = path.join(name);
            if config.exists() {
                return Some(config);
            }
        }
        path = path.parent()?.to_owned();
    }
}
