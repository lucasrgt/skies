//! Red through the real binary: how its revision is chosen and said, cases that never ran on red (whatever the
//! runner), and a red that does not match spec.md showing why.

mod support;

use support::{Repo, SPEC, new_spec, text};

fn short(repo: &Repo, rev: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short=7", rev])
        .current_dir(repo.path(""))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn red_names_its_revision_and_how_it_was_chosen() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let main = short(&repo, "main");

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded).contains(&format!(
            "  red    {main} (merge-base with main, no origin/HEAD; 1 commit before HEAD)\n"
        )),
        "{}",
        text(&recorded)
    );
    assert!(text(&recorded).contains("  time   red "), "{}", text(&recorded));

    // A long-lived branch the feature forked from: `default_branch` wins over main.
    repo.git(&["checkout", "--quiet", "-b", "develop", "main"]);
    repo.write("src/unrelated.txt", "develop\n");
    repo.git(&["commit", "--quiet", "-am", "develop moves on"]);
    repo.git(&["checkout", "--quiet", "feature"]);
    repo.git(&["rebase", "--quiet", "develop"]);
    let develop = short(&repo, "develop");
    let from_main = repo.skies(&["proof", "record", "1"]);
    assert!(
        text(&from_main).contains("(merge-base with main, no origin/HEAD; 2 commits before HEAD)"),
        "{}",
        text(&from_main)
    );
    let manifest = repo
        .read("Skies.toml")
        .replace("name = \"demo\"", "name = \"demo\"\ndefault_branch = \"develop\"");
    repo.write("Skies.toml", &manifest);
    repo.commit("default_branch");
    let configured = repo.skies(&["proof", "record", "1"]);
    assert!(configured.status.success(), "{}", text(&configured));
    assert!(
        text(&configured).contains(&format!(
            "  red    {develop} (merge-base with develop, default_branch in Skies.toml; 2 commits before HEAD)"
        )),
        "{}",
        text(&configured)
    );

    // An upstream is never consulted: only `default_branch` says where features fork from.
    repo.git(&["branch", "--quiet", "--set-upstream-to=main"]);
    let ignored = repo.skies(&["proof", "record", "1"]);
    assert!(
        text(&ignored).contains(&format!("  red    {develop} (merge-base with develop, default_branch")),
        "{}",
        text(&ignored)
    );

    let explicit = repo.skies(&["proof", "record", "1", "--red", "main"]);
    assert!(
        text(&explicit).contains(&format!("  red    {main} (--red main; 3 commits before HEAD)")),
        "{}",
        text(&explicit)
    );

    // `proof impact` without paths says what it diffs from, the same way.
    let impact = repo.skies(&["proof", "impact"]);
    assert!(
        text(&impact).starts_with(&format!(
            "changes since {develop} (merge-base with develop, default_branch in Skies.toml; 2 commits before HEAD)\n"
        )),
        "{}",
        text(&impact)
    );
}

#[test]
fn a_red_far_behind_head_is_flagged() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    for index in 0..55 {
        repo.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            &format!("unrelated {index}"),
        ]);
    }
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "a warning never fails: {}", text(&recorded));
    assert!(
        text(&recorded).contains("; 56 commits before HEAD)"),
        "{}",
        text(&recorded)
    );
    assert!(
        text(&recorded).contains("warning: the base is 56 commits before HEAD; if the feature started later"),
        "{}",
        text(&recorded)
    );
    let explicit = repo.skies(&["proof", "record", "1", "--red", "main"]);
    assert!(
        !text(&explicit).contains("warning"),
        "a named red is never second-guessed"
    );
    let impact = repo.skies(&["proof", "impact"]);
    assert!(
        text(&impact).contains("warning: the base is 56 commits"),
        "{}",
        text(&impact)
    );
}

/// A runner that, like vitest over a test file whose import does not exist yet, reports one failed file-level case
/// and exits 1 until the feature exists.
const LOAD_FAILURE: &str = r#"[ "$state" = on ] || {
  echo '<testsuites><testsuite name="vitest"><testcase name=".specs/0001-toggle/e2e/toggle.test.tsx"><failure message="Failed to resolve import ./Toggle"/></testcase></testsuite></testsuites>' > "$2"
  echo 'Error: Failed to resolve import "./Toggle" from "toggle.test.tsx"'
  exit 1
}"#;

#[test]
fn cases_that_never_ran_on_red_did_not_build_whatever_the_runner() {
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            "state=$(head -n 1 src/feature.txt)",
            &format!("state=$(head -n 1 src/feature.txt)\n{LOAD_FAILURE}"),
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded).contains(
            "red did not build because of the spec's own e2e (.specs/0001-toggle/e2e/toggle.test.tsx: Failed to \
             resolve import ./Toggle)"
        ),
        "{}",
        text(&recorded)
    );
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["failure_modes"]["FM-1"]["red"], "did-not-build");
    assert_eq!(receipt["failure_modes"]["FM-2"]["red"], "did-not-build");
    assert!(
        repo.read(&format!("{SPEC}/evidence/raw/red.log"))
            .contains("Failed to resolve import")
    );

    // Never on green: the same report there is a mismatch with spec.md, not a pass.
    repo.write("src/feature.txt", "off\n");
    let run = repo.skies(&["proof", "run", "1"]);
    assert_eq!(run.status.code(), Some(1), "{}", text(&run));
    assert!(text(&run).contains("does not match spec.md"), "{}", text(&run));
    assert!(
        text(&run).contains("Failed to resolve import"),
        "the output shows: {}",
        text(&run)
    );
}

#[test]
fn a_file_level_failure_outside_the_spec_is_not_did_not_build() {
    // The same shape of report, but the file that failed to load is a shared setup file, not the spec's own.
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            "state=$(head -n 1 src/feature.txt)",
            &format!(
                "state=$(head -n 1 src/feature.txt)\n{}",
                LOAD_FAILURE.replace(".specs/0001-toggle/e2e/toggle.test.tsx", "web.setup.ts")
            ),
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let refused = repo.skies(&["proof", "record", "1"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(
        text(&refused).contains("case \"web.setup.ts\" failed and is not one of the spec's e2e files"),
        "{}",
        text(&refused)
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}

#[test]
fn a_runner_build_runs_once_per_checkout_and_failing_on_red_did_not_build() {
    let repo = Repo::new();
    repo.write(
        "Skies.toml",
        "[workspace]\nname = \"demo\"\n\n[runners.fake]\nbuild = \"sh build.sh\"\n\
         command = \"sh run.sh {dir} {report} {evidence}\"\n",
    );
    // Counts builds outside the checkouts, and fails where the feature is off, as a compiler would.
    let counter = repo.path("builds.log");
    repo.write(
        "build.sh",
        &format!(
            "echo built >> {}\n[ \"$(head -n 1 src/feature.txt)\" = on ] || \
             {{ echo '.specs/0001-toggle/e2e/ToggleSpec.cs(3,5): error CS0246: Toggle'; exit 1; }}\n",
            counter.display()
        ),
    );
    repo.git(&["add", "Skies.toml", "build.sh"]);
    repo.git(&["commit", "--quiet", "-m", "build step"]);
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write(
        ".specs/0002-twin/spec.md",
        &support::spec_md("0002", &["FM-1 the twin stays off"]),
    );
    repo.write(".specs/0002-twin/e2e/cases.txt", "FM-1: twin toggles\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1", "--red", "HEAD~1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded)
            .contains("red did not build because of the spec's own e2e (.specs/0001-toggle/e2e/ToggleSpec.cs"),
        "{}",
        text(&recorded)
    );
    assert!(repo.read(&format!("{SPEC}/evidence/raw/red.log")).contains("CS0246"));
    assert_eq!(
        repo.read("builds.log").lines().count(),
        2,
        "one build on red, one on green"
    );

    let run = repo.skies(&["proof", "run", "2"]);
    assert!(run.status.success(), "{}", text(&run));
    assert_eq!(
        repo.read("builds.log").lines().count(),
        3,
        "one build per checkout per invocation"
    );
}

#[test]
fn a_red_that_does_not_match_the_spec_shows_its_output() {
    let repo = Repo::with_runner(|runner| format!("{runner}echo \"ran the cases in $1\"\n"));
    new_spec(&repo, "FM-1: toggles\nFM-3: not in the spec\n");
    repo.implement();

    let refused = repo.skies(&["proof", "record", "1"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    let output = text(&refused);
    assert!(output.contains("the red run does not match spec.md"), "{output}");
    assert!(
        output.contains("FM-2 is listed in spec.md but no test case names it"),
        "{output}"
    );
    assert!(output.contains("red's last output:"), "{output}");
    assert!(output.contains("ran the cases in .specs/0001-toggle/e2e"), "{output}");
    assert!(output.contains("--red <rev>"), "{output}");
    assert!(
        repo.read(&format!("{SPEC}/evidence/raw/red.log"))
            .contains("ran the cases"),
        "kept locally for a closer look"
    );
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
}
