//! The proof harness shared by the proof integration tests: a throwaway git repository with a shell-script runner
//! that writes JUnit. The runner passes a case only when `src/feature.txt` says `on`, so the base commit is a genuine
//! red and the working tree a genuine green. Cases titled `always` pass everywhere, `broken` fail everywhere,
//! `skip-on-red` are skipped without the feature and pass with it, and `skip-on-green` fail without it and are
//! skipped with it.
//!
//! For Assay-tagged failure modes, `<e2e>/verdicts.txt` lists `<fm number> <criterion> [always|never]`; the runner
//! writes each as a PascalCase, System.Text.Json-shaped verdict to `$SKIES_EVIDENCE/avp-FM-<n>.json`, passing with
//! the feature unless `always` or `never` says otherwise. It also drops `$SKIES_SPEC` into `spec.txt`, so tests can
//! see the automatic environment arrive.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

const RUNNER: &str = r#"#!/bin/sh
# usage: run.sh <e2e dir> <report> <evidence>; one case per line of <e2e dir>/cases.txt
state=$(head -n 1 src/feature.txt)
{
  echo '<testsuites><testsuite name="fake">'
  while IFS= read -r name; do
    [ -z "$name" ] && continue
    case "$name" in
      *always*) echo "<testcase name=\"$name\"/>" ;;
      *broken*) echo "<testcase name=\"$name\"><failure/></testcase>" ;;
      *skip-on-green*) if [ "$state" = on ]; then echo "<testcase name=\"$name\"><skipped/></testcase>"; else echo "<testcase name=\"$name\"><failure/></testcase>"; fi ;;
      *skip-on-red*) if [ "$state" = on ]; then echo "<testcase name=\"$name\"/>"; else echo "<testcase name=\"$name\"><skipped/></testcase>"; fi ;;
      *) if [ "$state" = on ]; then echo "<testcase name=\"$name\"/>"; else echo "<testcase name=\"$name\"><failure/></testcase>"; fi ;;
    esac
  done < "$1/cases.txt"
  echo '</testsuite></testsuites>'
} > "$2"
echo "screenshot" > "$3/final.txt"
echo "$SKIES_SPEC" > "$SKIES_EVIDENCE/spec.txt"
if [ -f "$1/verdicts.txt" ]; then
  while read -r n criterion mode; do
    [ -z "$n" ] && continue
    status=Fail
    [ "$state" = on ] && status=Pass
    [ "$mode" = always ] && status=Pass
    [ "$mode" = never ] && status=Fail
    printf '{"Subject":"toggle","Results":[{"CriterionId":"%s","Status":"%s","Reason":"r"}]}\n' "$criterion" "$status" > "$SKIES_EVIDENCE/avp-FM-$n.json"
  done < "$1/verdicts.txt"
fi
"#;

pub struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    pub fn new() -> Repo {
        let repo = Repo {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo.write(
            "Skies.toml",
            "[workspace]\nname = \"demo\"\n\n[runners.fake]\ncommand = \"sh run.sh {dir} {report} {evidence}\"\n",
        );
        repo.write("run.sh", RUNNER);
        repo.write("src/feature.txt", "off\n");
        repo.write("src/unrelated.txt", "x\n");
        repo.git(&["add", "."]);
        repo.git(&["commit", "--quiet", "-m", "base"]);
        repo.git(&["checkout", "--quiet", "-b", "feature"]);
        repo
    }

    /// Replaces the runner script (committed on main, so red sees it too) before the feature branch starts.
    pub fn with_runner(edit: impl Fn(&str) -> String) -> Repo {
        let repo = Repo::new();
        repo.git(&["checkout", "--quiet", "main"]);
        repo.write("run.sh", &edit(RUNNER));
        repo.git(&["commit", "--quiet", "-am", "change the runner"]);
        repo.git(&["checkout", "--quiet", "-B", "feature"]);
        repo
    }

    /// Commits the feature on the branch with the spec written so far, as an author would before recording.
    pub fn implement(&self) {
        self.write("src/feature.txt", "on\n");
        self.commit("implement the feature");
    }

    /// Commits everything in the working tree (what gitignore keeps out aside), so green is HEAD.
    pub fn commit(&self, message: &str) {
        self.git(&["add", "."]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    pub fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap()
    }

    pub fn json(&self, rel: &str) -> serde_json::Value {
        serde_json::from_str(&self.read(rel)).unwrap()
    }

    pub fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(self.dir.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    pub fn skies(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_skies"))
            .args(args)
            .current_dir(self.dir.path())
            .output()
            .unwrap()
    }

    pub fn worktrees(&self) -> usize {
        let output = Command::new("git")
            .args(["worktree", "list"])
            .current_dir(self.dir.path())
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).lines().count()
    }
}

pub fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A spec.md for spec `id` on the fake runner with the given failure-mode lines (without the leading dash).
pub fn spec_md(id: &str, fms: &[&str]) -> String {
    // These synthetic toggle cases exercise the engine itself, not an AVP domain protocol.
    let lines: Vec<String> = fms
        .iter()
        .map(|fm| {
            if fm.contains("[avp:") {
                format!("- {fm}")
            } else {
                format!("- {fm} [avp: none]")
            }
        })
        .collect();
    let exemptions: Vec<String> = fms.iter().filter(|fm| !fm.contains("[avp:")).map(|fm| {
        let id = fm.split_whitespace().next().unwrap();
        format!("- {id} Synthetic engine fixture: the toggle assertion directly decides this mode. | reviewed-by: fixture-reviewer")
    }).collect();
    format!(
        "---\nid: \"{id}\"\nrunner: fake\n---\n# Toggle\n\n## Failure modes\n\n{}\n",
        lines.join("\n")
    ) + "\n## AVP exemptions\n"
        + &exemptions.join("\n")
        + "\n"
}

pub fn spec_with(fms: &[&str]) -> String {
    spec_md("0001", fms)
}

pub const SPEC: &str = ".specs/0001-toggle";

/// `skies spec new toggle`, then two failure modes and the given cases.
pub fn new_spec(repo: &Repo, cases: &str) {
    let output = repo.skies(&["spec", "new", "toggle"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(repo.read(&format!("{SPEC}/spec.md")).contains("runner: fake"));
    assert!(repo.path(&format!("{SPEC}/e2e")).is_dir());
    repo.write(
        &format!("{SPEC}/spec.md"),
        &spec_with(&["FM-1 toggling does nothing", "FM-2 toggling twice breaks"]),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), cases);
}
