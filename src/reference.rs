use std::fmt;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::{RoamFile, settings::Settings};

/// A file-link destination and the Org-roam file that refers to it.
#[derive(Debug, PartialEq, Eq)]
pub struct FileReference {
    pub source: PathBuf,
    pub referenced: PathBuf,
}

impl FileReference {
    fn verify(&self, settings: &Settings) -> bool {
        if settings.exclude(&self.referenced) {
            return true;
        }
        if self.referenced.is_absolute() {
            return false;
        }
        let joined = self
            .source
            .parent()
            .expect("Should be within repository")
            .join(self.referenced.clone());
        joined.exists()
    }
}

/// An ID link whose destination is not in the Org-roam database.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingId {
    pub source: PathBuf,
    pub ref_id: String,
}

/// Findings from checking file and ID links.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub broken_links: Vec<FileReference>,
    pub unknown_ids: Vec<MissingId>,
}

/// A failed check, including file-link findings already collected before an ID error.
#[derive(Debug)]
pub struct CheckError {
    broken_links: Vec<FileReference>,
    error: anyhow::Error,
}

impl CheckError {
    /// Return any findings collected before the failure and the original error.
    pub fn into_parts(self) -> (Vec<FileReference>, anyhow::Error) {
        (self.broken_links, self.error)
    }
}

impl fmt::Display for CheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failed to check references")
    }
}

impl std::error::Error for CheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

/// Check links from the supplied database using the ignore paths in `settings`.
/// The caller is responsible for opening the intended database connection.
/// On an ID-query failure, the error retains any broken file links found first.
pub fn check(conn: &Connection, settings: &Settings) -> Result<Report, CheckError> {
    let broken_links = get_references(conn)
        .map_err(|error| CheckError {
            broken_links: Vec::new(),
            error,
        })?
        .into_iter()
        .filter(|reference| !reference.verify(settings))
        .collect();
    match external_ids(conn) {
        Ok(unknown_ids) => Ok(Report {
            broken_links,
            unknown_ids,
        }),
        Err(error) => Err(CheckError {
            broken_links,
            error,
        }),
    }
}

fn get_references(conn: &Connection) -> anyhow::Result<Vec<FileReference>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT nodes.file, links.dest
        FROM nodes JOIN links
        WHERE links.type = '"file"' AND nodes.id = links.source
    "#,
    )?;

    let references = stmt
        .query_map([], |row| {
            let source = row.get::<_, RoamFile>(0)?.as_ref().to_owned();
            let referenced = row.get::<_, RoamFile>(1)?.as_ref().to_owned();
            Ok(FileReference { source, referenced })
        })?
        .collect::<Result<_, _>>()?;

    Ok(references)
}

fn external_ids(conn: &Connection) -> anyhow::Result<Vec<MissingId>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT nodes.file, links.dest as ref_id
        FROM links JOIN nodes ON nodes.id = links.source
        WHERE type = '"id"' AND dest NOT IN (SELECT id FROM nodes)
    "#,
    )?;

    let ids = stmt
        .query_map([], |row| {
            let source = row.get::<_, RoamFile>(0)?.as_ref().to_owned();
            let ref_id = row.get(1)?;
            Ok(MissingId { source, ref_id })
        })?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}
