//! Repository tasks, run as `cargo xtask <task>`.
//!
//! - `frozen-fixtures [base]`: fail if this change edits or removes an
//!   existing line under `crates/proto/fixtures/`. Shapes are frozen; a
//!   shape change needs a new protocol version and its own fixture
//!   directory (`docs/versioning.md`). `base` defaults to the pull
//!   request's base branch; outside a pull request there is nothing to
//!   compare.

use std::process::{exit, Command};

const FIXTURES: &str = "crates/proto/fixtures/";

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("frozen-fixtures") => frozen_fixtures(args.next()),
        _ => {
            eprintln!("usage: cargo xtask frozen-fixtures [base-branch]");
            exit(2);
        }
    }
}

fn git(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("could not run git: {e}"));
    if !output.status.success() {
        eprintln!("git {} failed:", args.join(" "));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        exit(1);
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Lines a unified diff removes, without the file headers.
fn removed_lines(diff: &str) -> Vec<&str> {
    diff.lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .collect()
}

fn frozen_fixtures(base: Option<String>) {
    let base = base
        .or_else(|| std::env::var("GITHUB_BASE_REF").ok())
        .filter(|base| !base.is_empty());
    let Some(base) = base else {
        println!("frozen-fixtures: not a pull request, nothing to compare");
        return;
    };
    git(&["fetch", "--quiet", "--depth=1", "origin", &base]);
    let diff = git(&["diff", "--unified=0", "FETCH_HEAD", "HEAD", "--", FIXTURES]);
    let removed = removed_lines(&diff);
    if removed.is_empty() {
        println!("frozen-fixtures: no existing fixture line changed");
        return;
    }
    eprintln!(
        "An existing fixture line was changed or removed. Shapes are frozen: raise \
         PROTOCOL_VERSION and add fixtures/v<N>/ instead (docs/versioning.md)."
    );
    for line in removed {
        eprintln!("{line}");
    }
    exit(1);
}

#[cfg(test)]
mod tests {
    use super::removed_lines;

    #[test]
    fn only_removed_content_lines_count() {
        let diff = "\
diff --git a/x.jsonl b/x.jsonl
--- a/x.jsonl
+++ b/x.jsonl
@@ -2 +2 @@
-{\"old\":1}
+{\"new\":1}
@@ -9,0 +10 @@
+{\"added\":1}
";
        assert_eq!(removed_lines(diff), ["-{\"old\":1}"]);
        assert!(removed_lines("+++ b/x\n+{\"added\":1}\n").is_empty());
    }
}
