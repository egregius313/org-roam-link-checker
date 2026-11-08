use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use link_checker::reference::{self, FileReference, MissingId, Report};
use link_checker::settings::Settings;
use rusqlite::{Connection, params};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    db_path: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "link-checker-reference-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let db_path = root.join("org-roam.db");
        let source = root.join("note.org");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE nodes (id TEXT, file TEXT);\n             CREATE TABLE links (source TEXT, type TEXT, dest TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO nodes VALUES (?1, ?2)",
            params!["source", format!("\"{}\"", source.display())],
        )
        .unwrap();
        Self {
            root,
            db_path,
            source,
        }
    }

    fn conn(&self) -> Connection {
        Connection::open(&self.db_path).unwrap()
    }

    fn link(&self, link_type: &str, dest: &str) {
        self.conn()
            .execute(
                "INSERT INTO links VALUES (?1, ?2, ?3)",
                params!["source", format!("\"{link_type}\""), dest],
            )
            .unwrap();
    }

    fn file_link(&self, path: &str) {
        self.link("file", &format!("\"{path}\""));
    }

    fn settings(&self, ignored: &[&str]) -> Settings {
        Settings::load(
            Some(self.db_path.clone()),
            Some(self.root.join("no-config.yml")),
            ignored.iter().map(PathBuf::from).collect(),
        )
        .unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_link-checker"))
            .current_dir(&self.root)
            .env("HOME", &self.root)
            .arg("--roam-db")
            .arg(&self.db_path)
            .arg("--config")
            .arg(self.root.join("no-config.yml"))
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn report_contains_broken_files_and_unknown_ids() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("exists.org"), "").unwrap();
    fixture.file_link("exists.org");
    fixture.file_link("missing.org");
    fixture.file_link("ignored.org");
    fixture.link("id", "unknown-id");
    fixture.link("id", "known-id");
    fixture
        .conn()
        .execute(
            "INSERT INTO nodes VALUES (?1, ?2)",
            params!["known-id", format!("\"{}\"", fixture.source.display())],
        )
        .unwrap();

    let report = reference::check(&fixture.conn(), &fixture.settings(&["ignored.org"])).unwrap();
    assert_eq!(
        report,
        Report {
            broken_links: vec![FileReference {
                source: fixture.source.clone(),
                referenced: PathBuf::from("missing.org"),
            }],
            unknown_ids: vec![MissingId {
                source: fixture.source.clone(),
                ref_id: "unknown-id".into(),
            }],
        }
    );
}

#[test]
fn absolute_destinations_remain_broken_even_when_the_file_exists() {
    let fixture = Fixture::new();
    let target = fixture.root.join("exists.org");
    fs::write(&target, "").unwrap();
    fixture.file_link(target.to_str().unwrap());

    let report = reference::check(&fixture.conn(), &fixture.settings(&[])).unwrap();
    assert_eq!(
        report.broken_links,
        vec![FileReference {
            source: fixture.source.clone(),
            referenced: target,
        }]
    );
    assert!(report.unknown_ids.is_empty());
}

#[test]
fn empty_report_and_database_errors_are_returned_to_the_caller() {
    let fixture = Fixture::new();
    let settings = fixture.settings(&[]);
    let report = reference::check(&fixture.conn(), &settings).unwrap();
    assert_eq!(report, Report::default());

    let bad_database = Connection::open_in_memory().unwrap();
    assert!(reference::check(&bad_database, &settings).is_err());
}

#[test]
fn cli_prints_broken_files_and_unknown_ids_and_fails_for_broken_files() {
    let fixture = Fixture::new();
    fixture.file_link("missing.org");
    fixture.link("id", "unknown-id");

    let output = fixture.run(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "Broken link: {} -> missing.org\nFile {} has unknown id unknown-id\n",
            fixture.source.display(),
            fixture.source.display()
        )
    );
}

#[test]
fn cli_prints_unknown_ids_without_failing() {
    let fixture = Fixture::new();
    fixture.link("id", "unknown-id");

    let output = fixture.run(&[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "File {} has unknown id unknown-id\n",
            fixture.source.display()
        )
    );
}

#[test]
fn id_query_errors_keep_previously_found_file_links() {
    let fixture = Fixture::new();
    fixture.file_link("missing.org");
    fixture
        .conn()
        .execute(
            "INSERT INTO links VALUES (?1, ?2, ?3)",
            params!["source", "\"id\"", b"bad-id".as_slice()],
        )
        .unwrap();

    let error = reference::check(&fixture.conn(), &fixture.settings(&[])).unwrap_err();
    let (broken_links, cause) = error.into_parts();
    assert_eq!(
        broken_links,
        vec![FileReference {
            source: fixture.source.clone(),
            referenced: PathBuf::from("missing.org"),
        }]
    );
    assert!(!cause.to_string().is_empty());

    let output = fixture.run(&[]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("Broken link: {} -> missing.org\n", fixture.source.display())
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn cli_reports_database_errors_with_a_nonzero_status() {
    let fixture = Fixture::new();
    fixture.conn().execute("DROP TABLE links", []).unwrap();

    let output = fixture.run(&[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
