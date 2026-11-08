use std::env::{current_dir, home_dir};
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure};
use serde::Deserialize;
use serde_yml::from_reader;

/// The validated configuration used for a single run of the checker.
pub struct Settings {
    roam_db_path: PathBuf,
    ignored_paths: Vec<PathBuf>,
}

impl Settings {
    /// Resolve CLI overrides, a selected or discovered config file, and defaults.
    /// The CLI database path takes precedence over the config and default paths;
    /// CLI ignored paths are added to those from the config. Relative config
    /// database paths are resolved from the config file's directory.
    ///
    /// An unreadable or invalid selected config file is ignored for compatibility.
    /// Returns an error if no home directory is available for the default database
    /// or if the selected database does not exist.
    pub fn load(
        roam_db_path: Option<PathBuf>,
        config_file: Option<PathBuf>,
        ignored_paths: Vec<PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut builder = SettingsBuilder::new();
        if let Some(path) = config_file.or_else(find_ignore_file) {
            builder = builder.load_from_file(path);
        }
        if let Some(path) = roam_db_path {
            builder.roam_db_path = Some(path);
        }
        for path in ignored_paths {
            builder = builder.add_ignored_path(path);
        }
        builder.build()
    }

    pub fn roam_db(&self) -> &Path {
        &self.roam_db_path
    }

    pub fn exclude<P: AsRef<Path>>(&self, path: P) -> bool {
        self.ignored_paths
            .iter()
            .any(|ignored| ignored == path.as_ref())
    }
}

#[derive(Default)]
struct SettingsBuilder {
    roam_db_path: Option<PathBuf>,
    ignored_paths: Vec<PathBuf>,
}

impl SettingsBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn load_from_file<P: AsRef<Path>>(self, config_file: P) -> Self {
        match Config::from_file(config_file) {
            Ok(config) => self.load_config(config),
            Err(_) => self,
        }
    }

    fn load_config(mut self, config: Config) -> Self {
        if let Some(path) = config.roam_db {
            self.roam_db_path = Some(path);
        }
        self.ignored_paths.extend(config.ignored_paths);
        self
    }

    fn build(self) -> anyhow::Result<Settings> {
        let path = match self.roam_db_path {
            Some(path) => path,
            None => {
                let path = home_dir()
                    .ok_or_else(|| anyhow!("Could not determine home directory"))?
                    .join(".emacs.d/.local/cache/org-roam.db");
                ensure!(
                    path.exists(),
                    "Org-roam database not found at expected location: {}",
                    path.display()
                );
                path
            }
        };
        ensure!(
            path.exists(),
            "Org-roam database not found: {}",
            path.display()
        );
        Ok(Settings {
            roam_db_path: path,
            ignored_paths: self.ignored_paths,
        })
    }

    fn add_ignored_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.ignored_paths.push(path.as_ref().to_owned());
        self
    }
}

pub(crate) fn find_ignore_file() -> Option<PathBuf> {
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

#[derive(Deserialize, Default)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) ignored_paths: Vec<PathBuf>,
    #[serde(rename = "roam-db", default)]
    pub(crate) roam_db: Option<PathBuf>,
}

impl Config {
    pub(crate) fn from_file<P: AsRef<Path>>(file: P) -> anyhow::Result<Self> {
        let path = file.as_ref();
        let file = File::open(path)?;
        let mut config: Self = from_reader(file)?;
        if let Some(db_path) = config.roam_db.as_mut().filter(|path| path.is_relative()) {
            *db_path = path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(&*db_path);
        }
        Ok(config)
    }
}
