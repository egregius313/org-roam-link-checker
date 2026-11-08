# link-checker

Checks an Org-roam SQLite database for broken file links and IDs that do not
correspond to a node in the database. It reads the database rather than scanning
Org files directly, so sync the Org-roam database before running it.

## Usage

```text
link-checker [--roam-db <db-path>] [-i <ignore-file>]
```

The executable can be built and run with Cargo. Building requires Rust (edition
2024) and a SQLite development library.

The database is selected in this order:

1. `--roam-db <db-path>`, relative to the current working directory if not absolute.
2. The configuration file's `roam-db` value, relative to the configuration file if not absolute.
3. `~/.emacs.d/.local/cache/org-roam.db`.

The selected database must already exist; the checker will not create it or
fall back to another location if it cannot be opened.

Relative file-link destinations are checked relative to the source file's
directory. Absolute destinations are reported as broken unless ignored, even
if the target exists. Broken file links cause exit status 1. Unknown IDs are
also printed, but do not by themselves cause a nonzero exit status. Database
and other execution errors also produce a nonzero exit status.

## Configuration file

The optional YAML configuration file is named `.link-checker.yml`. The checker
looks in the current working directory, then each parent directory, and uses the
first file found. The legacy name `.link_checker.yml` is also supported; if both
names exist in the same directory, `.link-checker.yml` takes precedence.
Alternatively, specify a file explicitly:

```sh
link-checker -i ../.link-checker.yml
```

The long form is `--ignore-file-path`.

An explicit path is relative to the current working directory. If no file is
found, or the selected file cannot be read or parsed, the checker silently uses
an empty configuration. It does not fall back to another file after a selected
file fails to load.

### Database path

Set `roam-db` to select a database without supplying `--roam-db` each time:

```yaml
roam-db: data/org-roam.db
```

Relative paths are resolved from the directory containing the configuration
file. `~` and environment variables are not expanded. The `roam-db` and
`ignored_paths` fields are both optional and may be used together.

### Ignored paths

Use `ignored_paths` to list file-link destinations to exclude from checking:

```yaml
ignored_paths:
  - missing-document.pdf
  - ../shared/optional.org
  - /absolute/path/to/external-file
```

Each entry is compared against the destination stored in the database before it
is resolved relative to the source file. Entries are path matches, not globs or
directory prefixes: listing a directory does not ignore its contents. Paths are
not canonicalized, and `~` and environment variables are not expanded. Use the
same relative or absolute destination form as the link. Ignoring a file path
does not suppress unknown-ID reports.

For an empty list, use `ignored_paths: []`.
