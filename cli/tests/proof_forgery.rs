//! The ways a receipt could be forged or mislead, each refused through the real binary: a case skipped on red, a
//! red.patch that edits the spec's cases or what runs them, a `--red` whose diff changes the test setup, a green that
//! is not the commit the receipt names, a spec folder that shares its id with another, and a failure mode still
//! reading as the template's placeholder. Also what a receipt now pins: the runner's commands and a dirty green.

mod support;

use support::{Repo, SPEC, new_spec, spec_md, text};

/// A patch turning `src/feature.txt` off, plus whatever else `extra` adds.
fn off_patch(extra: &str) -> String {
    format!("--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1 +1 @@\n-on\n+off\n{extra}")
}

#[test]
fn a_red_patch_may_not_edit_the_specs_cases_or_what_runs_them() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: always toggles twice\n");
    repo.implement();
    // FM-2 passes on red; a patch that also renames its case so it fails there would forge a biting red.
    let rewrite = "--- a/.specs/0001-toggle/e2e/cases.txt\n+++ b/.specs/0001-toggle/e2e/cases.txt\n@@ -1,2 +1,2 @@\n \
                   FM-1: toggles\n-FM-2: always toggles twice\n+FM-2: toggles twice\n";
    let runner = "--- a/run.sh\n+++ b/run.sh\n@@ -1 +1 @@\n-#!/bin/sh\n+#!/bin/sh -e\n";
    for (extra, touched) in [(rewrite, ".specs/0001-toggle/e2e/cases.txt"), (runner, "run.sh")] {
        repo.write("off.patch", &off_patch(extra));
        let refused = repo.skies(&["proof", "record", "1", "--red-patch", "off.patch"]);
        assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
        assert!(
            text(&refused).contains(&format!(
                "off.patch touches {touched}. Red must differ from green in the feature alone"
            )),
            "{}",
            text(&refused)
        );
        assert!(
            !repo.path(&format!("{SPEC}/red.patch")).exists(),
            "a refused patch is never stored"
        );
    }
    // A stored red.patch is vetted too.
    std::fs::remove_file(repo.path("off.patch")).unwrap();
    repo.write(&format!("{SPEC}/red.patch"), &off_patch(rewrite));
    let stored = repo.skies(&["proof", "record", "1"]);
    assert_eq!(stored.status.code(), Some(2), "{}", text(&stored));
    assert!(
        text(&stored).contains("red.patch touches .specs/0001-toggle/e2e/cases.txt"),
        "{}",
        text(&stored)
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}

#[test]
fn an_explicit_red_whose_diff_changes_what_runs_the_tests_is_refused() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write(".specs/web.setup.ts", "export {};\n");
    repo.implement();
    let refused = repo.skies(&["proof", "record", "1", "--red", "main"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(
        text(&refused).contains(".specs/web.setup.ts changed. Those run the tests"),
        "{}",
        text(&refused)
    );
    // The default red takes the same diff as the branch's own, which review reads: a warning, not a refusal.
    let warned = repo.skies(&["proof", "record", "1"]);
    assert!(warned.status.success(), "{}", text(&warned));
    assert!(
        text(&warned)
            .contains("warning: since red, this branch also changed what runs the tests (.specs/web.setup.ts)"),
        "{}",
        text(&warned)
    );
}

#[test]
fn green_must_be_the_commit_the_receipt_names() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write("src/feature.txt", "on\n");
    repo.git(&["add", ".specs"]);
    repo.git(&["commit", "--quiet", "-m", "the spec alone"]);
    // The feature exists only in the working tree: HEAD would be red too.
    let refused = repo.skies(&["proof", "record", "1"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(
        text(&refused).contains("so green would not be the commit the receipt names:\n  src/feature.txt\n"),
        "{}",
        text(&refused)
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    let allowed = repo.skies(&["proof", "record", "1", "--allow-dirty"]);
    assert!(allowed.status.success(), "{}", text(&allowed));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["green"]["dirty"], true);

    // Committed, green is clean; what record wrote itself (receipt, evidence) never makes the tree dirty.
    repo.commit("implement");
    let clean = repo.skies(&["proof", "record", "1"]);
    assert!(clean.status.success(), "{}", text(&clean));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert!(receipt["green"].get("dirty").is_none(), "{receipt}");
    let again = repo.skies(&["proof", "record", "1"]);
    assert!(again.status.success(), "{}", text(&again));
}

#[test]
fn the_receipt_names_the_runner_commands_that_produced_it() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    let commands = &receipt["commands"];
    assert_eq!(commands["command"], "sh run.sh {dir} {report} {evidence}");
    assert!(
        commands.get("setup").is_none() && commands.get("build").is_none(),
        "{commands}"
    );
    let hash = commands["hash"].as_str().unwrap();
    assert!(
        hash.starts_with("blake3:") && hash.len() == "blake3:".len() + 64,
        "{hash}"
    );
}

#[test]
fn a_red_where_every_mode_did_not_build_says_what_it_proves() {
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
        text(&recorded).contains("warning: every failure mode is did-not-build, so this red proves only that"),
        "{}",
        text(&recorded)
    );
    assert_eq!(
        repo.json(&format!("{SPEC}/receipt.json"))["failure_modes"]["FM-1"]["red"],
        "did-not-build"
    );
}

#[test]
fn a_case_skipped_on_red_or_green_is_not_a_failure_or_a_pass() {
    // `it.skipIf(!featureExists)`: the case never runs on red, and a skip used to count as red failing.
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: skip-on-red toggles twice\n");
    repo.implement();
    let refused = repo.skies(&["proof", "record", "1"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(
        text(&refused).contains("FM-2: case \"FM-2: skip-on-red toggles twice\" was skipped on red"),
        "{}",
        text(&refused)
    );
    assert!(
        text(&refused).contains("must run and fail on red"),
        "{}",
        text(&refused)
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    repo.write(
        &format!("{SPEC}/e2e/cases.txt"),
        "FM-1: toggles\nFM-2: toggles twice\nFM-2: skip-on-green later\n",
    );
    repo.git(&["add", ".specs"]);
    repo.git(&["commit", "--quiet", "-m", "a case skipped with the feature"]);
    let skipped_green = repo.skies(&["proof", "record", "1"]);
    assert_eq!(skipped_green.status.code(), Some(1), "{}", text(&skipped_green));
    assert!(
        text(&skipped_green).contains("FM-2: case \"FM-2: skip-on-green later\" was skipped on green"),
        "{}",
        text(&skipped_green)
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}

#[test]
fn two_specs_sharing_an_id_are_refused_everywhere_and_spec_new_takes_the_next() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write(
        ".specs/0001-home/spec.md",
        &spec_md("0001", &["FM-1 the home is empty"]),
    );
    repo.write(".specs/0001-home/e2e/cases.txt", "FM-1: greets\n");
    repo.implement();
    for args in [
        &["proof", "run", "0001-home"][..],
        &["proof", "record", "1"],
        &["proof", "impact", "src/unrelated.txt"],
    ] {
        let refused = repo.skies(args);
        assert_eq!(refused.status.code(), Some(2), "{args:?}: {}", text(&refused));
        assert!(
            text(&refused).contains(".specs/0001-home and .specs/0001-toggle share id 0001"),
            "{args:?}: {}",
            text(&refused)
        );
    }
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    let created = repo.skies(&["spec", "new", "home-page"]);
    assert!(created.status.success(), "{}", text(&created));
    assert!(repo.path(".specs/0002-home-page/spec.md").is_file());
}

#[test]
fn a_spec_still_holding_the_template_placeholder_is_refused() {
    let repo = Repo::new();
    let created = repo.skies(&["spec", "new", "toggle"]);
    assert!(created.status.success(), "{}", text(&created));
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\n");
    repo.implement();
    for command in ["run", "record"] {
        let refused = repo.skies(&["proof", command, "1"]);
        assert_eq!(refused.status.code(), Some(1), "{command}: {}", text(&refused));
        assert!(
            text(&refused).contains("FM-1 still reads as the template's placeholder"),
            "{command}: {}",
            text(&refused)
        );
    }
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}
