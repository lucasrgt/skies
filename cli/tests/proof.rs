//! End to end through the real binary: a throwaway git repository, a shell-script runner that writes JUnit, and
//! the full spec new → record → status → verify loop. The runner passes a case only when `src/feature.txt` says
//! `on`, so the base commit is a genuine red and the working tree a genuine green.

use std::path::PathBuf;
use std::process::{Command, Output};

const RUNNER: &str = r#"#!/bin/sh
# usage: run.sh <e2e dir> <report>; one case per line of <e2e dir>/cases.txt
state=$(head -n 1 src/feature.txt)
{
  echo '<testsuites><testsuite name="fake">'
  while IFS= read -r name; do
    [ -z "$name" ] && continue
    case "$name" in
      *always*) echo "<testcase name=\"$name\"/>" ;;
      *) if [ "$state" = on ]; then echo "<testcase name=\"$name\"/>"; else echo "<testcase name=\"$name\"><failure/></testcase>"; fi ;;
    esac
  done < "$1/cases.txt"
  echo '</testsuite></testsuites>'
} > "$2"
echo "screenshot" > "$3/final.txt"
"#;

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Repo {
        let repo = Repo { dir: tempfile::tempdir().unwrap() };
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

    /// Commits the feature on the branch, as an author would before recording.
    fn implement(&self) {
        self.write("src/feature.txt", "on\n");
        self.git(&["commit", "--quiet", "-am", "implement the feature"]);
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap()
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(self.dir.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn skies(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_skies")).args(args).current_dir(self.dir.path()).output().unwrap()
    }

    fn worktrees(&self) -> usize {
        let output = Command::new("git").args(["worktree", "list"]).current_dir(self.dir.path()).output().unwrap();
        String::from_utf8_lossy(&output.stdout).lines().count()
    }
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn spec_with(fms: &[&str]) -> String {
    let lines: Vec<String> = fms.iter().map(|fm| format!("- {fm}")).collect();
    format!("---\nid: \"0001\"\nrunner: fake\n---\n# Toggle\n\n## Failure modes\n\n{}\n", lines.join("\n"))
}

const SPEC: &str = ".specs/0001-toggle";

fn new_spec(repo: &Repo, cases: &str) {
    let output = repo.skies(&["spec", "new", "toggle"]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(repo.read(&format!("{SPEC}/spec.md")).contains("runner: fake"));
    assert!(repo.path(&format!("{SPEC}/e2e")).is_dir());
    repo.write(&format!("{SPEC}/spec.md"), &spec_with(&["FM-1 toggling does nothing", "FM-2 toggling twice breaks"]));
    repo.write(&format!("{SPEC}/e2e/cases.txt"), cases);
}

#[test]
fn record_status_verify_round_trip() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\nhelper without an id\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert_eq!(repo.worktrees(), 1, "the red worktree is removed");

    let receipt: serde_json::Value = serde_json::from_str(&repo.read(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(receipt["spec"], "0001-toggle");
    assert_eq!(receipt["red"]["cases"], serde_json::json!({"FM-1": "fail", "FM-2": "fail"}));
    assert_eq!(receipt["green"]["cases"], serde_json::json!({"FM-1": "pass", "FM-2": "pass"}));
    assert_eq!(receipt["green"]["dirty"], false, "uncommitted spec files do not make green dirty");
    let footprint: Vec<&String> = receipt["footprint"].as_object().unwrap().keys().collect();
    assert_eq!(footprint, ["src/feature.txt"], "only what changed since the fork point, never the spec folder");
    assert!(receipt["inputs"].as_object().unwrap().contains_key(".specs/0001-toggle/e2e/cases.txt"));
    assert!(repo.path(&format!("{SPEC}/evidence/green.xml")).is_file());
    assert!(repo.path(&format!("{SPEC}/evidence/red.xml")).is_file());
    assert_eq!(repo.read(&format!("{SPEC}/evidence/final.txt")), "screenshot\n");

    let status = repo.skies(&["proof", "status"]);
    assert_eq!(text(&status), "0001-toggle  current\n");

    repo.write("src/feature.txt", "on\nrefactored\n");
    repo.write("src/unrelated.txt", "y\n");
    let status = repo.skies(&["proof", "status"]);
    assert!(status.status.success());
    assert_eq!(text(&status), "0001-toggle  stale (1 file changed: src/feature.txt)\n");

    let verified = repo.skies(&["proof", "verify", "--stale"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert!(text(&verified).contains("0001-toggle  verified  2/2 FMs pass"));
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
    let refreshed: serde_json::Value = serde_json::from_str(&repo.read(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(refreshed["red"], receipt["red"], "verify never touches red");
    assert!(repo.path(&format!("{SPEC}/evidence/red.xml")).is_file(), "verify keeps the red report");

    repo.write("src/feature.txt", "off\n");
    let broken = repo.skies(&["proof", "verify", "0001"]);
    assert_eq!(broken.status.code(), Some(1), "{}", text(&broken));
    assert!(text(&broken).contains("FM-1, FM-2 not passing"));
}

#[test]
fn a_case_that_passes_on_red_needs_a_justification() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: always true\n");
    repo.implement();

    let refused = repo.skies(&["proof", "record", "0001-toggle"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(text(&refused).contains("FM-2 already pass on red"));
    assert!(text(&refused).contains("## Non-discriminating"));
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    let spec = repo.read(&format!("{SPEC}/spec.md"));
    repo.write(
        &format!("{SPEC}/spec.md"),
        &format!("{spec}\n## Non-discriminating\n\n- FM-2 the flag is read-only.\n"),
    );
    let recorded = repo.skies(&["proof", "record", "0001"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt: serde_json::Value = serde_json::from_str(&repo.read(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(receipt["red"]["cases"]["FM-2"], "non-discriminating");
}

#[test]
fn inconsistent_cases_and_a_red_at_head_are_refused() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-3: not in the spec\n");
    repo.implement();

    let inconsistent = repo.skies(&["proof", "record", "1"]);
    assert_eq!(inconsistent.status.code(), Some(1));
    let message = text(&inconsistent);
    assert!(message.contains("FM-2 is listed in spec.md but no test case names it"), "{message}");
    assert!(message.contains("names FM-3"), "{message}");

    let at_head = repo.skies(&["proof", "record", "1", "--red", "HEAD"]);
    assert_eq!(at_head.status.code(), Some(2));
    assert!(text(&at_head).contains("--red-patch"));

    let nothing = repo.skies(&["proof", "verify"]);
    assert_eq!(nothing.status.code(), Some(2));
    assert!(text(&nothing).contains("--stale or --all"));
}

#[test]
fn a_red_patch_turns_head_into_red() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write("src/feature.txt", "on\n");
    let spec_files = [format!("{SPEC}/spec.md"), format!("{SPEC}/e2e/cases.txt")];
    let mut add = vec!["add", "src/feature.txt"];
    add.extend(spec_files.iter().map(String::as_str));
    repo.git(&add);
    repo.git(&["commit", "--quiet", "-m", "feature"]);
    repo.write("off.patch", "--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1 +1 @@\n-on\n+off\n");

    let recorded = repo.skies(&["proof", "record", "1", "--red-patch", "off.patch"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(repo.path(&format!("{SPEC}/red.patch")).is_file());
    let receipt: serde_json::Value = serde_json::from_str(&repo.read(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(receipt["red"]["patch"]["file"], "red.patch");
    assert_eq!(receipt["red"]["commit"], receipt["green"]["commit"]);
    let footprint: Vec<&String> = receipt["footprint"].as_object().unwrap().keys().collect();
    assert_eq!(footprint, ["src/feature.txt"]);

    // Without flags the stored red.patch is used again.
    let again = repo.skies(&["proof", "record", "1"]);
    assert!(again.status.success(), "{}", text(&again));
    assert!(text(&again).contains("+ red.patch"));
}
