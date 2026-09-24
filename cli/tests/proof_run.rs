//! The everyday loop through the real binary: `proof run` judges the cases now and commits nothing.

mod support;

use std::process::Command;

use support::{Repo, SPEC, new_spec, text};

fn porcelain(repo: &Repo) -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repo.path(""))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn run_judges_each_failure_mode_and_commits_nothing() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: always answers\n");

    let before = repo.skies(&["proof", "run", "1"]);
    assert_eq!(before.status.code(), Some(1), "{}", text(&before));
    let output = text(&before);
    assert!(output.contains("  FM-1  fail  toggling does nothing\n"), "{output}");
    assert!(output.contains("failed: FM-1: toggles"), "{output}");
    assert!(output.contains("  FM-2  pass  toggling twice breaks\n"), "{output}");
    assert!(output.contains("1 of 2 FMs fail"), "{output}");
    let committed = |repo: &Repo| -> Vec<String> {
        let evidence = repo.path(&format!("{SPEC}/evidence"));
        let mut names: Vec<String> = std::fs::read_dir(&evidence)
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    assert_eq!(committed(&repo), ["raw"], "run writes no committed evidence");
    assert!(
        repo.read(&format!("{SPEC}/evidence/raw/run.xml"))
            .contains("FM-1: toggles")
    );
    assert!(repo.path(&format!("{SPEC}/evidence/raw/run.log")).is_file());
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
    repo.git(&["add", ".specs"]);
    assert!(
        !porcelain(&repo).contains("evidence/raw"),
        "the local report is gitignored: {}",
        porcelain(&repo)
    );

    repo.implement();
    let after = repo.skies(&["proof", "run", "0001-toggle"]);
    assert!(after.status.success(), "{}", text(&after));
    assert!(text(&after).contains("2/2 FMs pass"), "{}", text(&after));
    assert_eq!(committed(&repo), ["raw"]);
}

#[test]
fn run_shows_what_a_failing_case_reported() {
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            r#"echo "<testcase name=\"$name\"><failure/></testcase>"; fi"#,
            r#"echo "<testcase name=\"$name\"><failure message=\"expected on, got $state\"/></testcase>"; fi"#,
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    let output = text(&repo.skies(&["proof", "run", "1"]));
    assert!(output.contains("  | expected on, got off"), "{output}");
}
