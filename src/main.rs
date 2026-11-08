use std::env::home_dir;
use std::path::PathBuf;

use anyhow::{Context, ensure};
use clap::Parser;
use link_checker::RoamFile;

pub fn resolve_org_roam_db_path() -> anyhow::Result<std::path::PathBuf> {
    let emacs_cache = home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".emacs.d")
        .join(".local")
        .join("cache");
    let db_path = emacs_cache.join("org-roam.db");
    ensure!(
        db_path.exists(),
        "Org-roam database not found at expected location: {}",
        db_path.display()
    );
    Ok(db_path)
}

struct Reference {
    source: PathBuf,
    referenced: PathBuf,
}

impl Reference {
    fn verify(&self, ignore: &link_checker::ignore_file::Ignore) -> bool {
        if ignore.exclude(&self.referenced) {
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

    /// Path to the ignore file
    #[arg(short, long)]
    ignore_file_path: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args {
        org_roam_db,
        ignore_file_path,
    } = Args::parse();

    let ignore = {
        use link_checker::ignore_file::{Ignore, find_ignore_file};
        ignore_file_path
            .or_else(find_ignore_file)
            .and_then(|path| Ignore::from_file(path).ok())
            .unwrap_or_default()
    };

    let db_path = match org_roam_db.or_else(|| ignore.roam_db_path().map(PathBuf::from)) {
        Some(path) => path,
        None => resolve_org_roam_db_path()?,
    };
    ensure!(
        db_path.exists(),
        "Org-roam database not found: {}",
        db_path.display()
    );
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .with_context(|| format!("Could not open Org-roam database: {}", db_path.display()))?;

    let references = get_references(&conn)?;

    let mut failed = false;
    for reference in references {
        if !reference.verify(&ignore) {
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
