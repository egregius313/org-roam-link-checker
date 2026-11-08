use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use link_checker::{RoamFile, settings::Settings};

struct Reference {
    source: PathBuf,
    referenced: PathBuf,
}

impl Reference {
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

fn get_references(conn: &rusqlite::Connection) -> anyhow::Result<Vec<Reference>> {
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
            Ok(Reference { source, referenced })
        })?
        .collect::<Result<_, _>>()?;

    Ok(references)
}

struct MissingId {
    source: PathBuf,
    ref_id: String,
}

/// Find id: links which reference an id which is not contained within the
/// org-roam database.
fn external_ids(conn: &rusqlite::Connection) -> anyhow::Result<Vec<MissingId>> {
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

/// Helper script to find potentially broken links and missing ids
#[derive(Parser)]
struct Args {
    /// Path to the org-roam database
    #[arg(long = "roam-db")]
    org_roam_db: Option<PathBuf>,

    /// Path to the configuration file
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// File-link destination to ignore (may be specified multiple times)
    #[arg(short = 'i', long = "ignore")]
    ignored_paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args {
        org_roam_db,
        config,
        ignored_paths,
    } = Args::parse();

    let settings = Settings::load(org_roam_db, config, ignored_paths)?;
    let db_path = settings.roam_db();
    let conn =
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
            .with_context(|| format!("Could not open Org-roam database: {}", db_path.display()))?;

    let references = get_references(&conn)?;

    let mut failed = false;
    for reference in references {
        if !reference.verify(&settings) {
            println!(
                "Broken link: {} -> {}",
                reference.source.display(),
                reference.referenced.display()
            );
            failed = true;
        }
    }

    for MissingId { source, ref_id } in external_ids(&conn)? {
        println!("File {} has unknown id {}", source.display(), ref_id);
    }

    if failed {
        std::process::exit(1);
    }
    Ok(())
}
