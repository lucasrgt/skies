//! Evidence as a spec's safety box, through the real binary: Assay verdicts that decide `[avp: …]` failure modes,
//! the automatic `SKIES_EVIDENCE`/`SKIES_SPEC` environment, and tamper detection over evidence/.

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
    let failing = repo.skies(&["proof", "record", "1"]);
    assert_eq!(failing.status.code(), Some(1), "{}", text(&failing));
    assert!(text(&failing).contains("key-honored is fail"), "{}", text(&failing));

    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 other-criterion\n");
    let wrong = repo.skies(&["proof", "record", "1"]);
    assert_eq!(wrong.status.code(), Some(1), "{}", text(&wrong));
    assert!(text(&wrong).contains("key-honored is not in the verdict"));

    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored\n");
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(text(&recorded).contains("[avp: key-honored]"));

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(
        receipt["green"]["cases"],
        json!({"FM-1": "pass", "FM-2": {"result": "pass", "avp": ["key-honored"], "verdict": "evidence/avp-FM-2.json"}})
    );
    assert_eq!(
        receipt["red"]["cases"],
        json!({"FM-1": "fail", "FM-2": {"result": "fail", "avp": ["key-honored"], "verdict": "evidence/red.avp-FM-2.json"}})
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

    // Verify holds a tagged mode to the same rule.
    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored never\n");
    let broken = repo.skies(&["proof", "verify", "1"]);
    assert_eq!(broken.status.code(), Some(1), "{}", text(&broken));
    assert!(text(&broken).contains("key-honored is fail"));
}

#[test]
fn a_case_passing_on_red_still_bites_when_its_verdict_fails_there() {
    let repo = Repo::new();
    tagged_spec(&repo, "FM-1: toggles\nFM-2: always answers\n", Some("2 key-honored\n"));
    repo.implement();

    let recorded = repo.skies(&["proof", "record", "1"]);

    assert!(recorded.status.success(), "{}", text(&recorded));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["red"]["cases"]["FM-2"]["result"], "fail");
}

#[test]
fn edited_evidence_is_tampered_not_stale() {
    let repo = Repo::new();
    tagged_spec(&repo, "FM-1: toggles\nFM-2: retries once\n", Some("2 key-honored\n"));
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    let hashed: Vec<&String> = receipt["evidence"].as_object().unwrap().keys().collect();
    assert_eq!(
        hashed,
        [
            "evidence/avp-FM-2.json",
            "evidence/final.txt",
            "evidence/green.xml",
            "evidence/red.avp-FM-2.json",
            "evidence/red.xml",
            "evidence/spec.txt"
        ]
    );
    let status = |repo: &Repo| text(&repo.skies(&["proof", "status"]));
    assert_eq!(status(&repo), "0001-toggle  current\n");

    // A verdict flipped by hand is the edit that matters most: the receipt no longer describes a run.
    let verdict = format!("{SPEC}/evidence/avp-FM-2.json");
    repo.write(&verdict, &repo.read(&verdict).replace("Pass", "Fail"));
    repo.write("src/feature.txt", "on\nedited\n");
    assert_eq!(
        status(&repo),
        "0001-toggle  tampered (1 file edited since recording: evidence/avp-FM-2.json)\n",
        "tampering is reported ahead of staleness"
    );

    // Verify re-proves green and rewrites its evidence, so the receipt is whole again.
    let verified = repo.skies(&["proof", "verify", "1"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert_eq!(status(&repo), "0001-toggle  current\n");

    // Red is never rerun, so an edit to red evidence survives verify: its recorded hash is kept.
    repo.write(&format!("{SPEC}/evidence/red.xml"), "<testsuites/>");
    repo.write(&format!("{SPEC}/evidence/extra.png"), "planted");
    let verified = repo.skies(&["proof", "verify", "1"]);
    assert!(verified.status.success(), "{}", text(&verified));
    assert_eq!(
        status(&repo),
        "0001-toggle  tampered (1 file edited since recording: evidence/red.xml)\n"
    );
    assert!(
        !repo.path(&format!("{SPEC}/evidence/extra.png")).exists(),
        "verify replaces the green evidence"
    );
}

#[test]
fn ignored_evidence_is_not_part_of_the_record() {
    let repo = Repo::new();
    tagged_spec(&repo, "FM-1: toggles\nFM-2: retries once\n", Some("2 key-honored\n"));
    repo.write(".specs/.gitignore", "final.txt\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));

    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert!(receipt["evidence"].get("evidence/final.txt").is_none());
    std::fs::remove_file(repo.path(&format!("{SPEC}/evidence/final.txt"))).unwrap();
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
}
