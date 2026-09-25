//! Evidence through the real binary: Assay verdicts that decide `[avp: …]` failure modes, the automatic
//! `SKIES_EVIDENCE`/`SKIES_SPEC` environment, and the size cap on committed artifacts.

mod support;

use serde_json::json;
use support::{Repo, SPEC, spec_with, text};

const TAGGED: [&str; 2] = [
    "FM-1 toggling does nothing",
    "FM-2 a retry toggles twice [avp: key-honored]",
];

/// A spec whose FM-2 is decided by an Assay verdict as well as its case; the verdict file is written by the runner
/// from `verdicts.txt` (absent unless given).
fn tagged_spec(repo: &Repo, cases: &str, verdicts: Option<&str>) {
    repo.write(&format!("{SPEC}/spec.md"), &spec_with(&TAGGED));
    repo.write(&format!("{SPEC}/e2e/cases.txt"), cases);
    if let Some(verdicts) = verdicts {
        repo.write(&format!("{SPEC}/e2e/verdicts.txt"), verdicts);
    }
}

#[test]
fn a_tagged_failure_mode_passes_only_with_a_passing_verdict() {
    let repo = Repo::new();
    tagged_spec(&repo, "FM-1: toggles\nFM-2: retries once\n", None);
    repo.implement();

    let missing = repo.skies(&["proof", "record", "1"]);
    assert_eq!(missing.status.code(), Some(1), "{}", text(&missing));
    assert!(
        text(&missing).contains("FM-2 [avp: key-honored] has no verdict"),
        "{}",
        text(&missing)
    );
    assert!(text(&missing).contains("$SKIES_EVIDENCE/avp-FM-2.json"));
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());

    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored never\n");
    repo.commit("verdicts");
    let failing = repo.skies(&["proof", "record", "1"]);
    assert_eq!(failing.status.code(), Some(1), "{}", text(&failing));
    assert!(text(&failing).contains("key-honored is fail"), "{}", text(&failing));

    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 other-criterion\n");
    repo.commit("verdicts");
    let wrong = repo.skies(&["proof", "record", "1"]);
    assert_eq!(wrong.status.code(), Some(1), "{}", text(&wrong));
    assert!(text(&wrong).contains("key-honored is not in the verdict"));

    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored\n");
    repo.commit("verdicts");
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(text(&recorded).contains("[avp: key-honored]"));

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(
        receipt["failure_modes"],
        json!({
            "FM-1": {"red": "fail", "green": "pass", "cases": ["FM-1: toggles"]},
            "FM-2": {
                "red": "fail", "green": "pass", "cases": ["FM-2: retries once"], "avp": ["key-honored"],
                "verdict": "evidence/avp-FM-2.json", "red_verdict": "evidence/red.avp-FM-2.json"
            }
        })
    );
    assert!(
        repo.read(&format!("{SPEC}/evidence/avp-FM-2.json"))
            .contains("\"Pass\"")
    );
    assert!(
        repo.read(&format!("{SPEC}/evidence/red.avp-FM-2.json"))
            .contains("\"Fail\"")
    );
    assert_eq!(
        repo.read(&format!("{SPEC}/evidence/spec.txt")),
        "0001-toggle\n",
        "SKIES_SPEC reaches the runner"
    );

    // `proof run` holds a tagged mode to the same rule.
    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored never\n");
    repo.commit("verdicts");
    let broken = repo.skies(&["proof", "run", "1"]);
    assert_eq!(broken.status.code(), Some(1), "{}", text(&broken));
    assert!(text(&broken).contains("key-honored is fail"), "{}", text(&broken));
}

#[test]
fn a_case_passing_on_red_still_bites_when_its_verdict_fails_there() {
    let repo = Repo::new();
    tagged_spec(&repo, "FM-1: toggles\nFM-2: always answers\n", Some("2 key-honored\n"));
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);

    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["failure_modes"]["FM-2"]["red"], "fail");
}

#[test]
fn an_oversized_committed_artifact_is_refused() {
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
}

#[test]
fn what_a_case_saves_under_raw_stays_local() {
    let repo = Repo::with_runner(|runner| {
        format!("{runner}mkdir -p \"$3/raw\" && head -c 300000 /dev/zero > \"$3/raw/trace.bin\"\n")
    });
    support::new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(repo.path(&format!("{SPEC}/evidence/raw/trace.bin")).is_file());
    assert!(!repo.path(&format!("{SPEC}/evidence/trace.bin")).exists());
}
