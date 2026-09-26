//! End to end through the real binary: spec new → record over the fake runner in `support`, with a genuine red at the
//! base commit and a genuine green in the working tree, and the receipt it leaves.

mod support;

use std::process::Command;

use serde_json::json;
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
fn record_writes_a_receipt_of_red_and_green_per_failure_mode() {
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            r#"echo "<testcase name=\"$name\"><failure/></testcase>"; fi"#,
            r#"echo "<testcase name=\"$name\"><failure message=\"expected on, got $state\"/></testcase>"; fi"#,
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\nhelper without an id\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert_eq!(repo.worktrees(), 1, "the red worktree is removed");

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    let red = String::from_utf8_lossy(
        &Command::new("git")
            .args(["rev-parse", "main"])
            .current_dir(repo.path(""))
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();
    assert_eq!(receipt["spec"], "0001-toggle");
    assert_eq!(receipt["runner"], "fake");
    assert_eq!(receipt["red"], json!({"commit": red}));
    assert_eq!(receipt["green"].as_object().unwrap().len(), 1, "green is its commit");
    assert_eq!(
        receipt["failure_modes"],
        json!({
            "FM-1": {"red": "fail", "green": "pass", "cases": ["FM-1: toggles"], "avp_exemption": {"reason": "Synthetic engine fixture: the toggle assertion directly decides this mode.", "reviewed_by": "fixture-reviewer"}, "message": "expected on, got off"},
            "FM-2": {"red": "fail", "green": "pass", "cases": ["FM-2: toggles twice"], "avp_exemption": {"reason": "Synthetic engine fixture: the toggle assertion directly decides this mode.", "reviewed_by": "fixture-reviewer"}, "message": "expected on, got off"}
        })
    );
    assert!(repo.path(&format!("{SPEC}/evidence/raw/green.xml")).is_file());
    assert!(repo.path(&format!("{SPEC}/evidence/raw/red.xml")).is_file());
    assert!(repo.path(&format!("{SPEC}/evidence/raw/red.log")).is_file());
    assert_eq!(
        repo.read(&format!("{SPEC}/evidence/final.txt")),
        "screenshot\n",
        "what the runner saves to {{evidence}} is committed"
    );
    assert!(repo.read(".specs/.gitignore").contains("/*/evidence/raw/"));
}

#[test]
fn recording_an_unchanged_spec_twice_changes_nothing() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let first = repo.skies(&["proof", "record", "1"]);
    assert!(first.status.success(), "{}", text(&first));
    repo.git(&["add", ".specs"]);
    let staged = porcelain(&repo);

    let second = repo.skies(&["proof", "record", "1"]);
    assert!(second.status.success(), "{}", text(&second));
    assert_eq!(
        porcelain(&repo),
        staged,
        "nothing differs from what the first record wrote"
    );
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
    repo.commit("justify FM-2");
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["failure_modes"]["FM-2"]["red"], "non-discriminating");
}

#[test]
fn inconsistent_cases_and_a_red_at_head_are_refused() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-3: not in the spec\n");
    repo.implement();

    let inconsistent = repo.skies(&["proof", "record", "1"]);
    assert_eq!(inconsistent.status.code(), Some(1));
    let message = text(&inconsistent);
    assert!(
        message.contains("FM-2 is listed in spec.md but no test case names it"),
        "{message}"
    );
    assert!(message.contains("names FM-3"), "{message}");

    let at_head = repo.skies(&["proof", "record", "1", "--red", "HEAD"]);
    assert_eq!(at_head.status.code(), Some(2));
    assert!(text(&at_head).contains("--red-patch"));
}

#[test]
fn a_red_patch_turns_head_into_red() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write("src/feature.txt", "on\n");
    repo.git(&["add", "src/feature.txt", ".specs"]);
    repo.git(&["commit", "--quiet", "-m", "feature"]);
    repo.write(
        "off.patch",
        "--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1 +1 @@\n-on\n+off\n",
    );

    let recorded = repo.skies(&["proof", "record", "1", "--red-patch", "off.patch"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(repo.path(&format!("{SPEC}/red.patch")).is_file());
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["red"]["patch"], "red.patch");
    assert_eq!(receipt["red"]["commit"], receipt["green"]["commit"]);

    // Without flags the stored red.patch is used again (the patch given before is no longer exempt from the clean
    // tree `record` asks for).
    std::fs::remove_file(repo.path("off.patch")).unwrap();
    let again = repo.skies(&["proof", "record", "1"]);
    assert!(again.status.success(), "{}", text(&again));
    assert!(text(&again).contains("+ red.patch"));

    // A patch that no longer applies says how to fix it.
    repo.write("src/feature.txt", "on\nmoved\n");
    repo.git(&["commit", "--quiet", "-am", "move the code"]);
    let rotted = repo.skies(&["proof", "record", "1"]);
    assert!(!rotted.status.success());
    assert!(
        text(&rotted).contains("no longer removes the feature"),
        "{}",
        text(&rotted)
    );
}

#[test]
fn e2e_that_does_not_build_on_red_counts_as_failing() {
    // A runner that, like `dotnet test` over E2E that reference code the feature adds, writes no report at all
    // until the feature exists.
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            "state=$(head -n 1 src/feature.txt)",
            "state=$(head -n 1 src/feature.txt)\n[ \"$state\" = on ] || { echo \"$PWD/$1/ToggleSpec.cs(3,5): error CS0246: type not found\"; exit 1; }",
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);

    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded).contains("red did not build because of the spec's own e2e ({root}/.specs/0001-toggle/e2e/"),
        "{}",
        text(&recorded)
    );
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["failure_modes"]["FM-1"]["red"], "did-not-build");
    assert_eq!(receipt["failure_modes"]["FM-2"]["red"], "did-not-build");
    assert_eq!(
        receipt["red"]["output"], "{root}/.specs/0001-toggle/e2e/ToggleSpec.cs(3,5): error CS0246: type not found",
        "the receipt says why red did not build, without the temporary checkout's path"
    );
    assert!(repo.read(&format!("{SPEC}/evidence/raw/red.log")).contains("CS0246"));
}

/// A runner that fails on red for a reason of its own, before any case of the spec could run.
fn red_breaks_with(output: &str) -> Repo {
    let failure =
        format!("state=$(head -n 1 src/feature.txt)\n[ \"$state\" = on ] || {{ echo \"{output}\"; exit 1; }}");
    let repo = Repo::with_runner(|runner| runner.replace("state=$(head -n 1 src/feature.txt)", &failure));
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    repo
}

#[test]
fn a_red_that_fails_for_another_reason_writes_no_receipt() {
    for (output, cause) in [
        (
            "$PWD/tests/App.Tests.csproj : error NU1101: Unable to find package Skies.Framework.Testing.",
            "an error outside the spec's e2e: ",
        ),
        (
            "$PWD/src/Toggle.cs(3,5): error CS0103: The name 'x' does not exist",
            "an error outside the spec's e2e: ",
        ),
        (
            "sh: 1: dotnet: not found",
            "nothing in its output points at the spec's e2e files",
        ),
    ] {
        let repo = red_breaks_with(output);
        let refused = repo.skies(&["proof", "record", "1"]);
        let message = text(&refused);
        assert_eq!(refused.status.code(), Some(2), "{message}");
        assert!(
            message
                .contains("red's cases never ran (the runner wrote no report), and not because of the spec's own e2e"),
            "{message}"
        );
        assert!(message.contains(cause), "{cause}: {message}");
        assert!(
            message.contains(output.split(": ").nth(1).unwrap_or(output)),
            "the output's tail: {message}"
        );
        assert!(message.contains("no receipt was written"), "{message}");
        assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
        assert!(
            repo.read(&format!("{SPEC}/evidence/raw/red.log"))
                .contains(output.split(' ').next_back().unwrap())
        );
        assert_eq!(repo.worktrees(), 1, "the red worktree is removed");
    }
}

#[test]
fn red_runs_inside_the_repository_so_repo_config_applies() {
    // A tool that reads its config from the parent directories (NuGet.config, .npmrc, global.json) must find the
    // repository's on red too. The runner prints the first `repo.conf` above its checkout; only the working tree has
    // one, so red finds it only when its checkout sits inside the repository.
    let repo = Repo::with_runner(|runner| {
        format!(
            "{runner}dir=$(cd .. && pwd); while [ \"$dir\" != / ] && [ ! -f \"$dir/repo.conf\" ]; do dir=$(dirname \"$dir\"); done\n\
             echo \"config: $dir/repo.conf\"\n"
        )
    });
    repo.write("repo.conf", "x\n");
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    let log = repo.read(&format!("{SPEC}/evidence/raw/red.log"));
    let found = std::fs::canonicalize(repo.path("repo.conf")).unwrap();
    assert!(
        log.contains(&format!("config: {}", found.display())),
        "red found the repository's config: {log}"
    );
    assert!(!repo.path(".skies-red").exists(), "the red checkout is cleaned up");
    assert_eq!(repo.worktrees(), 1);
    assert!(
        repo.read(".git/info/exclude").contains("/.skies-red/"),
        "git never shows it while it exists"
    );
}

#[test]
fn a_spec_without_failure_modes_is_refused() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\n");
    repo.write(
        &format!("{SPEC}/spec.md"),
        "---\nid: \"0001\"\nrunner: fake\n---\n# Toggle\n\n## Failure modes\n\nNone yet.\n",
    );
    repo.implement();
    for command in ["run", "record"] {
        let refused = repo.skies(&["proof", command, "1"]);
        assert_eq!(refused.status.code(), Some(1), "{command}: {}", text(&refused));
        assert!(
            text(&refused).contains("lists no failure mode, so there is nothing to prove"),
            "{}",
            text(&refused)
        );
        assert!(text(&refused).contains("`- FM-1 <what goes wrong>`"));
    }
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}

#[test]
fn look_alike_failure_modes_are_refused_with_the_grammar() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\n");
    repo.write(
        &format!("{SPEC}/spec.md"),
        &support::spec_with(&["FM-[rejected-update]: invalid input changes the entity"]),
    );
    let refused = repo.skies(&["proof", "run", "1"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(
        text(&refused).contains("is not a failure-mode line"),
        "{}",
        text(&refused)
    );

    repo.write(
        &format!("{SPEC}/spec.md"),
        &support::spec_with(&["FM-1 toggling does nothing"]),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\nFM 1: spaced\n");
    let look_alike = repo.skies(&["proof", "run", "1"]);
    assert_eq!(look_alike.status.code(), Some(1), "{}", text(&look_alike));
    assert!(
        text(&look_alike).contains("case \"FM 1: spaced\" starts like a failure-mode id"),
        "{}",
        text(&look_alike)
    );
}
