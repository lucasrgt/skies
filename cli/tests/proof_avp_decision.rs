//! AVP applicability must be decided before a runner can produce an accepted proof.
mod support;

use support::{Repo, SPEC, text};

fn feature(mode: &str, review: &str) -> Repo {
    let repo = Repo::new();
    repo.write(
        &format!("{SPEC}/spec.md"),
        &format!("---\nid: 0001\nrunner: fake\n---\n## Failure modes\n- FM-1 {mode}\n{review}"),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\n");
    repo.implement();
    repo
}

#[test]
fn omitted_decision_cannot_run_or_record_even_when_cases_pass() {
    let repo = feature("toggling does nothing", "");
    for command in ["run", "record"] {
        let result = repo.skies(&["proof", command, "1"]);
        assert_eq!(result.status.code(), Some(1), "{}", text(&result));
        assert!(text(&result).contains("AVP decision"), "{}", text(&result));
    }
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
    assert!(!repo.path(&format!("{SPEC}/evidence/raw/run.log")).exists());
}

#[test]
fn exemption_requires_both_rationale_and_named_review() {
    for review in [
        "",
        "## AVP exemptions\n- FM-1 | reviewed-by: fixture-reviewer\n",
        "## AVP exemptions\n- FM-1 This fixture checks a local toggle directly.\n",
    ] {
        let repo = feature("toggling does nothing [avp: none]", review);
        let result = repo.skies(&["proof", "run", "1"]);
        assert!(!result.status.success(), "{}", text(&result));
        assert!(text(&result).contains("AVP exemption"), "{}", text(&result));
        assert!(!repo.path(&format!("{SPEC}/evidence/raw/run.log")).exists());
    }
}

#[test]
fn reviewed_exemption_is_preserved_in_the_receipt() {
    let reason =
        "The synthetic toggle fixture has no vendor or domain protocol; its state assertion decides this mode.";
    let repo = feature(
        "toggling does nothing [avp: none]",
        &format!("## AVP exemptions\n- FM-1 {reason} | reviewed-by: fixture-reviewer\n"),
    );
    let result = repo.skies(&["proof", "record", "1"]);
    assert!(result.status.success(), "{}", text(&result));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(receipt["failure_modes"]["FM-1"]["avp_exemption"]["reason"], reason);
    assert_eq!(
        receipt["failure_modes"]["FM-1"]["avp_exemption"]["reviewed_by"],
        "fixture-reviewer"
    );
}

#[test]
fn ambiguous_tags_and_mixed_none_are_rejected() {
    for tag in ["[avp: none, key-honored]", "[avp: key-honored] [avp: none]"] {
        let repo = feature(&format!("toggling does nothing {tag}"), "");
        let result = repo.skies(&["proof", "run", "1"]);
        assert!(!result.status.success());
        assert!(!repo.path(&format!("{SPEC}/evidence/raw/run.log")).exists());
    }
}

#[test]
fn invalid_review_records_cannot_exempt_a_mode() {
    for review in [
        "- FM-1 pending | reviewed-by: fixture-reviewer",
        "- FM-1 A direct assertion decides this fixture. | reviewed-by: <actual reviewer>",
        "- FM-2 This mode does not exist. | reviewed-by: fixture-reviewer",
        "- FM-1 Direct fixture assertion. | reviewed-by: fixture-reviewer\n- FM-1 Another reason. | reviewed-by: fixture-reviewer",
    ] {
        let repo = feature(
            "toggling does nothing [avp: none]",
            &format!("## AVP exemptions\n{review}\n"),
        );
        let result = repo.skies(&["proof", "record", "1"]);
        assert!(!result.status.success(), "{}", text(&result));
        assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
    }
    let repo = feature(
        "toggling does nothing [avp: key-honored]",
        "## AVP exemptions\n- FM-1 Cannot waive a required verifier. | reviewed-by: fixture-reviewer\n",
    );
    assert!(!repo.skies(&["proof", "run", "1"]).status.success());
}
