use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, params};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "link-checker-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn database(&self, relative: &str, marker: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE nodes (id TEXT, file TEXT);\n             CREATE TABLE links (source TEXT, type TEXT, dest TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO nodes VALUES (?1, ?2)",
            params!["source", format!("\"{}/source.org\"", self.root.display())],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links VALUES (?1, ?2, ?3)",
            params!["source", "\"id\"", marker],
        )
        .unwrap();
        path
    }

    fn config(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_link-checker"))
            .current_dir(cwd)
            .env("HOME", &self.root)
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

fn assert_marker(output: Output, marker: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(marker));
}

#[test]
fn settings_resolve_cli_overrides_and_merge_ignored_paths() {
    let fixture = Fixture::new();
    let db = fixture.database("cli.db", "cli-id");
    let config = fixture.root.join("config/custom.yml");
    fixture.config(
        "config/custom.yml",
        "roam-db: config.db\nignored_paths:\n  - config.org\n",
    );
    let settings = link_checker::settings::Settings::load(
        Some(db.clone()),
        Some(config),
        vec!["cli.org".into()],
    )
    .unwrap();

    assert_eq!(settings.roam_db(), db);
    assert!(settings.exclude("config.org"));
    assert!(settings.exclude("cli.org"));
    assert!(!settings.exclude("other.org"));
}

#[test]
fn settings_resolve_config_database_relative_to_its_file() {
    let fixture = Fixture::new();
    let db = fixture.database("config/configured.db", "config-id");
    let config = fixture.root.join("config/custom.yml");
    fixture.config("config/custom.yml", "roam-db: configured.db\n");

    let settings = link_checker::settings::Settings::load(None, Some(config), vec![]).unwrap();
    assert_eq!(settings.roam_db(), db);
}

#[test]
fn command_line_overrides_config_and_default() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    fixture.database("configured.db", "config-id");
    fixture.database("cli.db", "cli-id");
    fixture.config(".link-checker.yml", "roam-db: configured.db\n");

    let output = fixture.run(&fixture.root, &["--roam-db", "cli.db"]);
    assert_marker(output, "cli-id");
}

#[test]
fn config_only_database_resolves_relative_to_config_file() {
    let fixture = Fixture::new();
    fixture.database("config/configured.db", "config-id");
    fixture.config("config/.link-checker.yml", "roam-db: configured.db\n");
    let cwd = fixture.root.join("config/subdir");
    fs::create_dir_all(&cwd).unwrap();

    assert_marker(fixture.run(&cwd, &[]), "config-id");
}

#[test]
fn explicit_config_supports_roam_db_and_ignored_paths_together() {
    let fixture = Fixture::new();
    let db = fixture.database("config/configured.db", "config-id");
    let conn = Connection::open(db).unwrap();
    conn.execute(
        "INSERT INTO links VALUES (?1, ?2, ?3)",
        params!["source", "\"file\"", "\"missing.org\""],
    )
    .unwrap();
    fixture.config(
        "config/custom.yml",
        "roam-db: configured.db\nignored_paths:\n  - missing.org\n",
    );

    assert_marker(
        fixture.run(&fixture.root, &["--config", "config/custom.yml"]),
        "config-id",
    );
}

#[test]
fn command_line_ignore_paths_extend_configured_paths() {
    let fixture = Fixture::new();
    let db = fixture.database("config/configured.db", "config-id");
    let conn = Connection::open(db).unwrap();
    for path in ["config.org", "short.org", "long.org", "reported.org"] {
        conn.execute(
            "INSERT INTO links VALUES (?1, ?2, ?3)",
            params!["source", "\"file\"", format!("\"{path}\"")],
        )
        .unwrap();
    }
    fixture.config(
        "config/custom.yml",
        "roam-db: configured.db\nignored_paths:\n  - config.org\n",
    );
    let args = [
        "--config",
        "config/custom.yml",
        "-i",
        "short.org",
        "--ignore",
        "long.org",
    ];
    let output = fixture.run(&fixture.root, &args);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reported.org"));
    assert!(!stdout.contains("config.org"));
    assert!(!stdout.contains("short.org"));
    assert!(!stdout.contains("long.org"));

    let mut args = args.to_vec();
    args.extend(["-i", "reported.org"]);
    assert_marker(fixture.run(&fixture.root, &args), "config-id");
}

#[test]
fn command_line_ignore_works_without_a_config_file() {
    let fixture = Fixture::new();
    let db = fixture.database("configured.db", "id");
    let conn = Connection::open(db).unwrap();
    conn.execute(
        "INSERT INTO links VALUES (?1, ?2, ?3)",
        params!["source", "\"file\"", "\"missing.org\""],
    )
    .unwrap();

    assert_marker(
        fixture.run(
            &fixture.root,
            &["--roam-db", "configured.db", "--ignore", "missing.org"],
        ),
        "id",
    );
}

#[test]
fn legacy_config_name_remains_supported() {
    let fixture = Fixture::new();
    fixture.database("configured.db", "legacy-id");
    fixture.config(".link_checker.yml", "roam-db: configured.db\n");

    assert_marker(fixture.run(&fixture.root, &[]), "legacy-id");
}

#[test]
fn default_database_is_used_without_configuration() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");

    assert_marker(fixture.run(&fixture.root, &[]), "default-id");
}

#[test]
fn missing_selected_database_fails_without_creating_a_file() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    let output = fixture.run(&fixture.root, &["--roam-db", "missing.db"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing.db"));
    assert!(!fixture.root.join("missing.db").exists());
}

#[test]
fn missing_configured_database_does_not_fall_back_or_create_a_file() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    fixture.config("config/custom.yml", "roam-db: missing.db\n");

    let output = fixture.run(&fixture.root, &["--config", "config/custom.yml"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing.db"));
    assert!(!fixture.root.join("config/missing.db").exists());
}

#[test]
fn absolute_database_path_in_config_is_used_directly() {
    let fixture = Fixture::new();
    let db = fixture.database("configured.db", "absolute-id");
    fixture.config("config/custom.yml", &format!("roam-db: {}\n", db.display()));

    assert_marker(
        fixture.run(&fixture.root, &["--config", "config/custom.yml"]),
        "absolute-id",
    );
}

#[test]
fn ignore_only_config_preserves_default_database() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    fixture.config(".link_checker.yml", "ignored_paths: []\n");

    assert_marker(fixture.run(&fixture.root, &[]), "default-id");
}

#[test]
fn unreadable_explicit_config_uses_defaults_without_discovery() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    fixture.database("configured.db", "config-id");
    fixture.config(".link-checker.yml", "roam-db: configured.db\n");

    assert_marker(
        fixture.run(&fixture.root, &["--config", "missing.yml"]),
        "default-id",
    );
}

#[test]
fn malformed_explicit_config_keeps_command_line_overrides() {
    let fixture = Fixture::new();
    let db = fixture.database("cli.db", "cli-id");
    let conn = Connection::open(db).unwrap();
    conn.execute(
        "INSERT INTO links VALUES (?1, ?2, ?3)",
        params!["source", "\"file\"", "\"missing.org\""],
    )
    .unwrap();
    fixture.config("bad.yml", "ignored_paths: [invalid\n");

    assert_marker(
        fixture.run(
            &fixture.root,
            &[
                "--config",
                "bad.yml",
                "--roam-db",
                "cli.db",
                "-i",
                "missing.org",
            ],
        ),
        "cli-id",
    );
}

#[test]
fn malformed_discovered_config_uses_defaults_without_searching_parents() {
    let fixture = Fixture::new();
    fixture.database(".emacs.d/.local/cache/org-roam.db", "default-id");
    fixture.database("configured.db", "config-id");
    fixture.config(".link-checker.yml", "roam-db: configured.db\n");
    fixture.config("nested/.link-checker.yml", "ignored_paths: [invalid\n");

    assert_marker(fixture.run(&fixture.root.join("nested"), &[]), "default-id");
}

#[test]
fn nearest_config_wins_and_hyphenated_name_wins_in_same_directory() {
    let fixture = Fixture::new();
    fixture.database("parent.db", "parent-id");
    fixture.database("nested/hyphen.db", "hyphen-id");
    fixture.database("nested/legacy.db", "legacy-id");
    fixture.config(".link-checker.yml", "roam-db: parent.db\n");
    fixture.config("nested/.link-checker.yml", "roam-db: hyphen.db\n");
    fixture.config("nested/.link_checker.yml", "roam-db: legacy.db\n");
    let cwd = fixture.root.join("nested/deeper");
    fs::create_dir_all(&cwd).unwrap();

    assert_marker(fixture.run(&cwd, &[]), "hyphen-id");
}
