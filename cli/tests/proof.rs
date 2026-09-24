//! End to end through the real binary: the full spec new → record → status → verify loop over the fake runner in
//! `support`, with a genuine red at the base commit and a genuine green in the working tree.

mod support;

use support::{Repo, SPEC, new_spec, text};

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
    assert_eq!(
        receipt["red"]["cases"],
        serde_json::json!({
            "FM-1": {"result": "fail", "cases": ["FM-1: toggles"]},
            "FM-2": {"result": "fail", "cases": ["FM-2: toggles twice"]}
        })
    );
    assert_eq!(
        receipt["green"]["cases"],
        serde_json::json!({
            "FM-1": {"result": "pass", "cases": ["FM-1: toggles"]},
            "FM-2": {"result": "pass", "cases": ["FM-2: toggles twice"]}
        })
    );
    assert_eq!(
        receipt["green"]["dirty"], false,
        "uncommitted spec files do not make green dirty"
    );
    let footprint: Vec<&String> = receipt["footprint"].as_object().unwrap().keys().collect();
    assert_eq!(
        footprint,
        ["src/feature.txt"],
        "only what changed since the fork point, never the spec folder"
    );
    assert!(
        receipt["inputs"]
            .as_object()
            .unwrap()
            .contains_key(".specs/0001-toggle/e2e/cases.txt")
    );
    assert_eq!(receipt["red"]["report"]["file"], "evidence/raw/red.xml");
    assert_eq!(receipt["green"]["report"]["file"], "evidence/raw/green.xml");
    assert!(repo.path(&format!("{SPEC}/evidence/raw/green.xml")).is_file());
    assert!(repo.path(&format!("{SPEC}/evidence/raw/red.xml")).is_file());
    assert!(
        !repo.path(&format!("{SPEC}/evidence/red.xml")).exists(),
        "full reports are not committed evidence"
    );
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
    assert!(
        repo.path(&format!("{SPEC}/evidence/raw/red.xml")).is_file(),
        "verify keeps the red report"
    );

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
    assert_eq!(receipt["red"]["cases"]["FM-2"]["result"], "non-discriminating");
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
    repo.write(
        "off.patch",
        "--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1 +1 @@\n-on\n+off\n",
    );

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

#[test]
fn e2e_that_does_not_build_on_red_counts_as_failing() {
    // A runner that, like `dotnet test` over E2E that reference code the feature adds, writes no report at all
    // until the feature exists.
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            "state=$(head -n 1 src/feature.txt)",
            "state=$(head -n 1 src/feature.txt)\n[ \"$state\" = on ] || { echo 'error CS0246: type not found'; exit 1; }",
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);

    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(text(&recorded).contains("red did not build"));
    let receipt: serde_json::Value = serde_json::from_str(&repo.read(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(
        receipt["red"]["cases"],
        serde_json::json!({"FM-1": {"result": "did-not-build"}, "FM-2": {"result": "did-not-build"}})
    );
    assert_eq!(receipt["red"]["report"]["file"], "evidence/raw/red.log");
    assert_eq!(
        receipt["red"]["output"], "error CS0246: type not found",
        "the receipt says why red did not build"
    );
    assert!(repo.read(&format!("{SPEC}/evidence/raw/red.log")).contains("CS0246"));
}
