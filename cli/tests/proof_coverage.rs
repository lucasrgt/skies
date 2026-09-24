//! The coverage footprint through the real binary: a fake runner that writes LCOV for the files listed in
//! `covers.txt`, the way vitest or `flutter test --coverage` would, and the fallback to the diff without it.

mod support;

use support::{Repo, SPEC, new_spec, text};

/// After the report, write one LCOV record per line of covers.txt (absolute paths, as vitest writes them), plus one
/// file outside the project and one that never ran, which must both stay out of the footprint.
const LCOV: &str = r#"if [ -f covers.txt ]; then
  {
    while IFS= read -r file; do
      [ -z "$file" ] && continue
      printf 'SF:%s\nDA:1,1\nend_of_record\n' "$PWD/$file"
    done < covers.txt
    printf 'SF:/elsewhere/framework/Lib.cs\nDA:1,4\nend_of_record\n'
    printf 'SF:src/unrelated.txt\nDA:1,0\nend_of_record\n'
  } > "$SKIES_COVERAGE"
fi
echo "screenshot" > "$3/final.txt""#;

/// A repository whose feature relies on `src/shared.txt` without changing it.
fn covered_repo() -> Repo {
    let repo = Repo::with_runner(|runner| runner.replace(r#"echo "screenshot" > "$3/final.txt""#, LCOV));
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("src/shared.txt", "middleware\n");
    repo.write("covers.txt", "src/feature.txt\nsrc/shared.txt\n");
    repo.git(&["add", "src/shared.txt", "covers.txt"]);
    repo.git(&["commit", "--quiet", "-m", "shared code"]);
    repo.git(&["checkout", "--quiet", "-B", "feature"]);
    repo
}

fn footprint(repo: &Repo) -> Vec<String> {
    repo.json(&format!("{SPEC}/receipt.json"))["footprint"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn the_footprint_holds_the_files_green_executed() {
    let repo = covered_repo();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded).contains("footprint from coverage: 2 executed + 1 changed since red + touches = 2 files"),
        "{}",
        text(&recorded)
    );
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["footprint_source"], "coverage");
    assert_eq!(receipt["footprint_changed"], serde_json::json!(["src/feature.txt"]));
    assert_eq!(
        footprint(&repo),
        ["src/feature.txt", "src/shared.txt"],
        "the shared file it never changed is in; the unexecuted and outside ones are not"
    );
    let evidence: Vec<String> = std::fs::read_dir(repo.path(&format!("{SPEC}/evidence")))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !evidence
            .iter()
            .any(|name| name.contains("lcov") || name.contains("coverage")),
        "{evidence:?}"
    );

    repo.write("src/unrelated.txt", "y\n");
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
    repo.write("src/shared.txt", "middleware, rewritten\n");
    assert_eq!(
        text(&repo.skies(&["proof", "status"])),
        "0001-toggle  stale (1 file changed: src/shared.txt)\n"
    );
    let impact = repo.skies(&["proof", "impact", "src/shared.txt"]);
    assert!(text(&impact).contains("0001-toggle  stale"), "{}", text(&impact));
    assert!(text(&impact).contains("via src/shared.txt"), "{}", text(&impact));
}

#[test]
fn verify_refreshes_the_executed_files_and_keeps_the_changed_ones() {
    let repo = covered_repo();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));

    // The code evolves: green no longer runs shared.txt nor feature.txt itself, but a new helper instead.
    repo.write("src/helper.txt", "helper\n");
    repo.write("covers.txt", "src/helper.txt\n");
    let verified = repo.skies(&["proof", "verify", "1"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert!(
        text(&verified).contains("verified  2/2 FMs pass (footprint 2 files, coverage)"),
        "{}",
        text(&verified)
    );
    assert_eq!(
        footprint(&repo),
        ["src/feature.txt", "src/helper.txt"],
        "the executed part follows today's run; the change the receipt proves stays"
    );
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["footprint_changed"], serde_json::json!(["src/feature.txt"]));
    repo.write("src/shared.txt", "no longer executed\n");
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
}

#[test]
fn without_coverage_the_footprint_falls_back_to_the_diff_and_says_so() {
    let repo = covered_repo();
    repo.git(&["rm", "--quiet", "covers.txt"]);
    repo.git(&["commit", "--quiet", "-m", "no coverage"]);
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded)
            .contains("footprint from the diff since red + touches (2 files): runner 'fake' writes no coverage"),
        "{}",
        text(&recorded)
    );
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["footprint_source"], "diff");
    assert!(receipt.get("footprint_changed").is_none());
    assert_eq!(
        footprint(&repo),
        ["covers.txt", "src/feature.txt"],
        "what changed since red, as before"
    );

    // Coverage turns up later: verify upgrades the receipt, keeping what it had recorded as the change.
    repo.write("covers.txt", "src/shared.txt\n");
    let verified = repo.skies(&["proof", "verify", "1"]);
    assert!(verified.status.success(), "{}", text(&verified));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["footprint_source"], "coverage");
    assert_eq!(footprint(&repo), ["covers.txt", "src/feature.txt", "src/shared.txt"]);
}

#[test]
fn a_declared_coverage_path_that_stays_empty_is_named() {
    let repo = Repo::new();
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write(
        "Skies.toml",
        "[workspace]\nname = \"demo\"\n\n[runners.fake]\ncommand = \"sh run.sh {dir} {report} {evidence}\"\ncoverage = \"out/lcov.info\"\n",
    );
    repo.git(&["commit", "--quiet", "-am", "declare coverage"]);
    repo.git(&["checkout", "--quiet", "-B", "feature"]);
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(
        text(&recorded).contains("runner 'fake' wrote no coverage at"),
        "{}",
        text(&recorded)
    );
    assert!(text(&recorded).contains("out/lcov.info"), "{}", text(&recorded));
    assert_eq!(repo.json(&format!("{SPEC}/receipt.json"))["footprint_source"], "diff");
}
