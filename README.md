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

The checker expects the database at `~/.emacs.d/.local/cache/org-roam.db`.
Although `--roam-db <PATH>` is accepted by the CLI, it is currently ignored;
the default location is always used.

Relative file-link destinations are checked relative to the source file's
directory. Absolute destinations are reported as broken unless ignored, even
if the target exists. Broken file links cause exit status 1. Unknown IDs are
also printed, but do not by themselves cause a nonzero exit status. Database
and other execution errors also produce a nonzero exit status.

## Configuration file

The optional YAML configuration file is named `.link_checker.yml`. The checker
looks in the current working directory, then each parent directory, and uses the
first file found. Alternatively, specify a file explicitly:

```sh
link-checker -i ../.link_checker.yml
```

The long form is `--ignore-file-path`.

An explicit path is relative to the current working directory. If no file is
found, or the selected file cannot be read or parsed, the checker silently uses
an empty ignore list. It does not fall back to another file after a selected
file fails to load.

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
