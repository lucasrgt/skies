//! The coverage footprint through the real binary: a fake runner that writes LCOV for the files listed in
//! `covers.txt`, the way vitest or `flutter test --coverage` would, and the fallback to the diff without it.

mod support;

use support::{Repo, SPEC, new_spec, text};

/// After the report, write one LCOV record per line of covers.txt (absolute paths, as vitest writes them), plus one
/// file outside the project and one that never ran, which must both stay out of the footprint. A line is a path and
/// optionally the line numbers the run executed (`src/app.txt 2 4`), line 1 when it names none.
const LCOV: &str = r#"if [ -f covers.txt ]; then
  {
    while read -r file executed; do
      [ -z "$file" ] && continue
      printf 'SF:%s\n' "$PWD/$file"
      for n in ${executed:-1}; do printf 'DA:%s,1\n' "$n"; done
      printf 'DA:999,0\nend_of_record\n'
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
        text(&recorded).contains(
            "footprint from coverage: 2 executed + 1 changed since red + touches = 2 files (2 pinned by executed lines)"
        ),
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
        text(&verified).contains("verified  2/2 FMs pass (footprint 2 files, coverage, 1 by executed lines)"),
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

/// Five lines; green executes 2 and 4 (a handler body) and never 1, 3, or 5.
const APP: &str = "header\nrun one\nnever run\nrun two\nfooter\n";

fn status(repo: &Repo) -> String {
    text(&repo.skies(&["proof", "status"]))
}

/// A recorded receipt whose green run executed only lines 2 and 4 of `src/app.txt`.
fn recorded_app() -> Repo {
    let repo = covered_repo();
    repo.write("src/app.txt", APP);
    repo.write("covers.txt", "src/feature.txt\nsrc/app.txt 2 4\n");
    repo.git(&["add", "src/app.txt", "covers.txt"]);
    repo.git(&["commit", "--quiet", "-m", "app"]);
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    repo
}

#[test]
fn a_covered_file_is_pinned_by_the_lines_green_executed() {
    let repo = recorded_app();
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    let app = &receipt["footprint"]["src/app.txt"];
    assert_eq!(app["lines"], "2,4", "{receipt:#}");
    assert!(app["hash"].as_str().unwrap().starts_with("blake3:"));
    assert_eq!(receipt["footprint"]["src/feature.txt"]["lines"], "1");
    let current = "0001-toggle  current\n";
    let stale = "0001-toggle  stale (1 file changed: src/app.txt)\n";
    assert_eq!(status(&repo), current);

    let edits = [
        (
            APP.replace("never run", "never run, edited"),
            current,
            "a line green never ran",
        ),
        (
            APP.replace("footer", "footer, edited\nand more"),
            current,
            "lines past the last executed one",
        ),
        (APP.replace('\n', "\r\n"), current, "line endings"),
        (
            APP.replace("run two", "run  two"),
            stale,
            "an executed line, whitespace included",
        ),
        (
            format!("inserted\n{APP}"),
            stale,
            "a line inserted above shifts the executed ones",
        ),
        (
            "header\nrun one\nnever run\n".to_string(),
            stale,
            "the file shrank below line 4",
        ),
    ];
    for (edited, expected, what) in edits {
        repo.write("src/app.txt", &edited);
        assert_eq!(status(&repo), expected, "{what}");
        let impact = text(&repo.skies(&["proof", "impact", "src/app.txt"]));
        let state = if expected == current { "current" } else { "stale" };
        assert!(impact.contains(&format!("0001-toggle  {state}")), "{what}: {impact}");
    }
    std::fs::remove_file(repo.path("src/app.txt")).unwrap();
    assert_eq!(status(&repo), stale, "a deleted file");
    repo.write("src/app.txt", APP);
    assert_eq!(status(&repo), current);

    // verify re-pins from its own run: green now executes line 3 instead.
    repo.write("covers.txt", "src/feature.txt\nsrc/app.txt 3\n");
    let verified = repo.skies(&["proof", "verify", "1"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert_eq!(
        repo.json(&format!("{SPEC}/receipt.json"))["footprint"]["src/app.txt"]["lines"],
        "3"
    );
    repo.write("src/app.txt", &APP.replace("run two", "no longer run"));
    assert_eq!(status(&repo), current);
    repo.write("src/app.txt", &APP.replace("never run", "now run"));
    assert_eq!(status(&repo), stale);
}

#[test]
fn a_receipt_with_whole_file_hashes_for_covered_files_still_reads() {
    let repo = recorded_app();
    // As a receipt written before executed lines: every footprint file by its whole-file hash.
    let path = format!("{SPEC}/receipt.json");
    let mut receipt = repo.json(&path);
    for (file, print) in receipt["footprint"].as_object_mut().unwrap() {
        let bytes = std::fs::read(repo.path(file)).unwrap();
        *print = serde_json::json!(format!("blake3:{}", blake3::hash(&bytes).to_hex()));
    }
    repo.write(&path, &serde_json::to_string_pretty(&receipt).unwrap());
    assert_eq!(status(&repo), "0001-toggle  current\n");
    repo.write("src/app.txt", &APP.replace("never run", "never run, edited"));
    assert_eq!(
        status(&repo),
        "0001-toggle  stale (1 file changed: src/app.txt)\n",
        "a whole-file hash goes stale on any edit, as it always did"
    );
}
