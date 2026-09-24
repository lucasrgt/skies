//! Compact evidence through the real binary: full reports stay local under a gitignored evidence/raw/, the receipt
//! keeps their summary, re-proving an unchanged spec leaves git clean, oversized committed artifacts are refused,
//! a red.patch that no longer applies is reported as red rot and fixed with `record --red-only`, and receipts from
//! before compact evidence still read and migrate.

mod support;

use std::process::Command;

use serde_json::json;
use support::{Repo, SPEC, spec_with, text};

fn porcelain(repo: &Repo) -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repo.path(""))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn commit_all(repo: &Repo, message: &str) {
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
}

/// A runner whose failures say why and whose report and Assay verdict carry timings that change on every run.
fn timed_repo() -> Repo {
    let repo = Repo::with_runner(|runner| {
        runner
            .replace(
                r#"echo "<testcase name=\"$name\"><failure/></testcase>"; fi"#,
                r#"echo "<testcase name=\"$name\" time=\"1$(date +%N)\"><failure message=\"expected on, got $state\"/></testcase>"; fi"#,
            )
            .replace(
                r#"echo '<testsuites><testsuite name="fake">'"#,
                r#"echo "<testsuites><testsuite name=\"fake\" timestamp=\"1$(date +%N)\">""#,
            )
            .replace(
                r#""Reason":"r"}]}\n' "$criterion" "$status""#,
                r#""Reason":"r","DurationMs":%s}]}\n' "$criterion" "$status" "1$(date +%N)""#,
            )
    });
    repo.write(
        &format!("{SPEC}/spec.md"),
        &spec_with(&[
            "FM-1 toggling does nothing",
            "FM-2 a retry toggles twice [avp: key-honored]",
        ]),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\nFM-2: retries once\n");
    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored\n");
    repo
}

#[test]
fn full_reports_stay_local_and_the_receipt_keeps_their_summary() {
    let repo = timed_repo();
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(
        receipt["red"]["cases"]["FM-1"],
        json!({"result": "fail", "cases": ["FM-1: toggles"], "message": "expected on, got off"})
    );
    assert_eq!(receipt["red"]["report"]["file"], "evidence/raw/red.xml");
    assert!(
        receipt["red"]["report"]["hash"]
            .as_str()
            .unwrap()
            .starts_with("blake3:")
    );
    assert!(repo.read(".specs/.gitignore").contains("/*/evidence/raw/"));

    repo.git(&["add", "-A"]);
    let staged = porcelain(&repo);
    assert!(!staged.contains("evidence/raw"), "{staged}");
    assert!(staged.contains("evidence/avp-FM-2.json"), "{staged}");
    assert!(staged.contains(".specs/.gitignore"), "{staged}");
}

#[test]
fn re_proving_an_unchanged_spec_changes_no_committed_file() {
    let repo = timed_repo();
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    commit_all(&repo, "record");

    for args in [["proof", "verify", "--all"], ["proof", "verify", "--all"]] {
        let verified = repo.skies(&args);
        assert!(
            text(&verified).contains("verified (current, unchanged)"),
            "{}",
            text(&verified)
        );
        assert_eq!(porcelain(&repo), "", "a current receipt is left byte for byte");
    }

    // A refresh re-anchors green to today's HEAD (the commit above); a second one has nothing left to change, even
    // though the report's and the verdict's timings differ on every run.
    let first = repo.skies(&["proof", "verify", "1", "--refresh"]);
    assert!(first.status.success(), "{}", text(&first));
    let receipt = repo.read(&format!("{SPEC}/receipt.json"));
    let verdict = repo.read(&format!("{SPEC}/evidence/avp-FM-2.json"));
    let changed = porcelain(&repo);
    assert_eq!(
        changed, " M .specs/0001-toggle/receipt.json\n",
        "only the receipt moves, to the new green commit"
    );
    let second = repo.skies(&["proof", "verify", "1", "--refresh"]);
    assert!(second.status.success(), "{}", text(&second));
    assert_eq!(
        repo.read(&format!("{SPEC}/receipt.json")),
        receipt,
        "no byte of the receipt changes"
    );
    assert_eq!(
        repo.read(&format!("{SPEC}/evidence/avp-FM-2.json")),
        verdict,
        "nor of the verdict"
    );
    assert_eq!(porcelain(&repo), changed);
}

#[test]
fn an_oversized_committed_artifact_is_refused_until_it_is_ignored() {
    let repo = Repo::with_runner(|runner| format!("{runner}head -c 300000 /dev/zero > \"$3/trace.bin\"\n"));
    support::new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let refused = repo.skies(&["proof", "record", "1"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    let message = text(&refused);
    assert!(message.contains("capped at 256 KB"), "{message}");
    assert!(message.contains("evidence/trace.bin is 292 KB"), "{message}");
    assert!(message.contains("$SKIES_EVIDENCE/raw/"), "{message}");
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    let rules = repo.read(".specs/.gitignore");
    repo.write(".specs/.gitignore", &format!("{rules}/*/evidence/trace.bin\n"));
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert!(receipt["evidence"].get("evidence/trace.bin").is_none());
    assert!(
        repo.path(&format!("{SPEC}/evidence/trace.bin")).is_file(),
        "kept locally"
    );
}

/// A feature committed on HEAD and a red.patch that turns it off, recorded and committed.
fn retro_spec() -> Repo {
    let repo = Repo::new();
    support::new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write("src/feature.txt", "on\n");
    commit_all(&repo, "feature");
    repo.write(
        "off.patch",
        "--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1 +1 @@\n-on\n+off\n",
    );
    let recorded = repo.skies(&["proof", "record", "1", "--red-patch", "off.patch"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    std::fs::remove_file(repo.path("off.patch")).unwrap();
    commit_all(&repo, "record");
    repo
}

#[test]
fn a_red_patch_that_no_longer_applies_is_red_rotted() {
    let repo = retro_spec();
    let status = |repo: &Repo| text(&repo.skies(&["proof", "status"]));
    assert_eq!(status(&repo), "0001-toggle  current\n");
    assert!(
        repo.path(".git/skies/red-rot.json").is_file(),
        "the check is cached in the git directory"
    );

    // The code under the patch moves: green still passes, but the patch no longer applies.
    repo.write("src/feature.txt", "on\nrefactored\n");
    let rotted = status(&repo);
    assert!(
        rotted.starts_with(
            "0001-toggle  stale (1 file changed: src/feature.txt), red-rotted (red.patch no longer applies)\n"
        ),
        "{rotted}"
    );
    assert!(rotted.contains("--red-only"), "{rotted}");
    assert_eq!(status(&repo), rotted, "a cached result reads the same");

    repo.write("src/feature.txt", "on\n");
    assert_eq!(status(&repo), "0001-toggle  current\n", "the check follows the files");
}

#[test]
fn record_red_only_rewrites_only_the_red_half() {
    let repo = retro_spec();
    let before = repo.json(&format!("{SPEC}/receipt.json"));
    repo.write("src/feature.txt", "on\nrefactored\n");
    commit_all(&repo, "refactor");

    let stale_red = repo.skies(&["proof", "record", "1", "--red-only"]);
    assert_eq!(stale_red.status.code(), Some(2), "{}", text(&stale_red));
    assert!(text(&stale_red).contains("red rotted"), "{}", text(&stale_red));

    repo.write(
        "off.patch",
        "--- a/src/feature.txt\n+++ b/src/feature.txt\n@@ -1,2 +1 @@\n-on\n-refactored\n+off\n",
    );
    let fixed = repo.skies(&["proof", "record", "1", "--red-only", "--red-patch", "off.patch"]);
    assert!(fixed.status.success(), "{}", text(&fixed));
    assert!(text(&fixed).contains("rewrote the red half"), "{}", text(&fixed));

    let after = repo.json(&format!("{SPEC}/receipt.json"));
    assert_ne!(after["red"]["commit"], before["red"]["commit"]);
    assert_ne!(after["red"]["patch"]["hash"], before["red"]["patch"]["hash"]);
    for kept in ["green", "footprint", "inputs"] {
        assert_eq!(after[kept], before[kept], "{kept} is kept");
    }
    let status = text(&repo.skies(&["proof", "status"]));
    assert!(!status.contains("red-rotted"), "{status}");

    let unrecorded = Repo::new();
    support::new_spec(&unrecorded, "FM-1: toggles\nFM-2: toggles twice\n");
    unrecorded.implement();
    let refused = unrecorded.skies(&["proof", "record", "1", "--red-only"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert!(text(&refused).contains("has no receipt yet"), "{}", text(&refused));
}

#[test]
fn a_receipt_that_committed_its_reports_still_reads_and_migrates_on_refresh() {
    let repo = timed_repo();
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));

    // Rewrite it as Skies 5 wrote receipts before compact evidence: bare entries, reports committed and hashed.
    for report in ["red.xml", "green.xml"] {
        std::fs::rename(
            repo.path(&format!("{SPEC}/evidence/raw/{report}")),
            repo.path(&format!("{SPEC}/evidence/{report}")),
        )
        .unwrap();
    }
    let mut old = repo.json(&format!("{SPEC}/receipt.json"));
    old["red"]["report"] = json!("evidence/red.xml");
    old["green"]["report"] = json!("evidence/green.xml");
    old["red"]["cases"]["FM-1"] = json!("fail");
    old["green"]["cases"]["FM-1"] = json!("pass");
    old["red"]["cases"]["FM-2"].as_object_mut().unwrap().remove("cases");
    let hash = |rel: &str| {
        format!(
            "blake3:{}",
            blake3::hash(&std::fs::read(repo.path(rel)).unwrap()).to_hex()
        )
    };
    for report in ["red.xml", "green.xml"] {
        old["evidence"][format!("evidence/{report}")] = json!(hash(&format!("{SPEC}/evidence/{report}")));
    }
    repo.write(
        &format!("{SPEC}/receipt.json"),
        &serde_json::to_string_pretty(&old).unwrap(),
    );

    let status = text(&repo.skies(&["proof", "status"]));
    assert!(status.starts_with("0001-toggle  current\n"), "{status}");
    assert!(status.contains("1 receipt still commit"), "{status}");

    let migrated = repo.skies(&["proof", "verify", "1", "--refresh"]);
    assert!(migrated.status.success(), "{}", text(&migrated));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["red"]["report"]["file"], "evidence/raw/red.xml");
    assert_eq!(
        receipt["red"]["cases"]["FM-1"],
        json!({"result": "fail", "cases": ["FM-1: toggles"], "message": "expected on, got off"}),
        "red's summary is read back from the committed report"
    );
    assert_eq!(receipt["red"]["cases"]["FM-2"]["cases"], json!(["FM-2: retries once"]));
    assert!(!repo.path(&format!("{SPEC}/evidence/red.xml")).exists());
    assert!(!repo.path(&format!("{SPEC}/evidence/green.xml")).exists());
    assert!(repo.path(&format!("{SPEC}/evidence/raw/red.xml")).is_file());
    let hashed: Vec<&String> = receipt["evidence"].as_object().unwrap().keys().collect();
    assert!(hashed.iter().all(|path| !path.ends_with(".xml")), "{hashed:?}");
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
}
