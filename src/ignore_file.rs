//! Compatibility interface for callers using the original ignore-file module.
//! New code should use [`crate::settings::Settings`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::settings::Config;

#[derive(Default)]
pub struct Ignore {
    config: Config,
}

impl<'de> Deserialize<'de> for Ignore {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Config::deserialize(deserializer).map(|config| Self { config })
    }
}

impl Ignore {
    pub fn from_file<P: AsRef<Path>>(file: P) -> anyhow::Result<Self> {
        Ok(Self {
            config: Config::from_file(file)?,
        })
    }

    pub fn roam_db_path(&self) -> Option<&Path> {
        self.config.roam_db.as_deref()
    }

    pub fn add_paths<I: IntoIterator<Item = PathBuf>>(&mut self, paths: I) {
        self.config.ignored_paths.extend(paths);
    }

    pub fn exclude<P: AsRef<Path>>(&self, path: P) -> bool {
        for ignored_path in &self.config.ignored_paths {
            if ignored_path == path.as_ref() {
                return true;
            }
        }
        false
    }
}

pub fn find_ignore_file() -> Option<PathBuf> {
    crate::settings::find_ignore_file()
}
