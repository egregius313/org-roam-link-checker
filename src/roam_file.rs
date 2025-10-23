use rusqlite::types::{FromSql, ToSql};
use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

/// A wrapper around [`PathBuf`] to represent a file in the org-roam system.
///
/// The path is always absolute.
///
/// The path is stored in the database with double quotes around it, so when converting to and from SQL,
/// we need to add or remove the quotes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RoamFile(PathBuf);

impl std::fmt::Display for RoamFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl AsRef<Path> for RoamFile {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl<T: Into<PathBuf>> From<T> for RoamFile {
    fn from(p: T) -> Self {
        #[inline(always)]
        fn try_from<T: Into<PathBuf>>(p: T) -> Result<RoamFile, std::io::Error> {
            let path: PathBuf = p.into();

            if path.is_absolute() {
                return Ok(RoamFile(path));
            }
            let file_name = path.canonicalize()?;
            let expanded_path = current_dir()?.join(file_name);
            Ok(RoamFile(expanded_path))
        }
        match try_from(p) {
            Ok(rf) => rf,
            Err(e) => panic!("Failed to convert to RoamFile: {}", e),
        }
    }
}

impl ToSql for RoamFile {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let path = format!(r#""{}""#, self.0.to_str().unwrap());
        Ok(rusqlite::types::ToSqlOutput::from(path))
    }
}

impl FromSql for RoamFile {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        use rusqlite::types::{FromSqlError::InvalidType, ValueRef::Text};

        let Text(s) = value else {
            return Err(InvalidType);
        };

        let s = std::str::from_utf8(s).map_err(|_| InvalidType)?;
        let s = s
            .strip_prefix('"')
            .ok_or(InvalidType)?
            .strip_suffix('"')
            .ok_or(InvalidType)?;
        Ok(RoamFile(PathBuf::from(s)))
    }
}
