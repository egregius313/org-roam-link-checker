use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use link_checker::{reference, settings::Settings};

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

    let (broken_links, unknown_ids) = match reference::check(&conn, &settings) {
        Ok(report) => (report.broken_links, Ok(report.unknown_ids)),
        Err(error) => {
            let (broken_links, error) = error.into_parts();
            (broken_links, Err(error))
        }
    };

    for reference in &broken_links {
        println!(
            "Broken link: {} -> {}",
            reference.source.display(),
            reference.referenced.display()
        );
    }

    for missing in unknown_ids? {
        println!(
            "File {} has unknown id {}",
            missing.source.display(),
            missing.ref_id
        );
    }

    if !broken_links.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
