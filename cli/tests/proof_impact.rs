//! The receipts as an impact index, through the real binary: `skies proof impact` and `record --with-impacted`.

mod support;

use support::{Repo, SPEC, spec_md, text};

const TWIN: &str = ".specs/0002-twin";

/// Two specs over the same feature file: 0001 recorded, 0002 written and ready to record.
fn two_specs() -> Repo {
    let repo = Repo::new();
    repo.write(
        &format!("{SPEC}/spec.md"),
        &spec_md(
            "0001",
            &[
                "FM-1 toggling does nothing",
                "FM-2 a retry toggles twice [avp: key-honored]",
            ],
        ),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\nFM-2: retries once\n");
    repo.write(&format!("{SPEC}/e2e/verdicts.txt"), "2 key-honored\n");
    repo.write(
        &format!("{TWIN}/spec.md"),
        &spec_md("0002", &["FM-1 the twin stays off"]),
    );
    repo.write(&format!("{TWIN}/e2e/cases.txt"), "FM-1: twin toggles\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    repo
}

#[test]
fn impact_lists_the_specs_a_path_reaches_with_their_failure_modes() {
    let repo = two_specs();
    repo.write(
        ".specs/0003-other/spec.md",
        "---\nid: \"0003\"\nrunner: fake\ntouches: [src/unrelated.txt]\n---\n## Failure modes\n- FM-1 other\n",
    );

    let by_path = repo.skies(&["proof", "impact", "src/feature.txt"]);
    assert!(by_path.status.success(), "{}", text(&by_path));
    assert_eq!(
        text(&by_path),
        "0001-toggle  current\n  via src/feature.txt\n  - FM-1 toggling does nothing\n  \
         - FM-2 a retry toggles twice [avp: key-honored]\n\
         1 spec impacted; baseline before changing code: skies proof verify 0001\n"
    );

    let by_glob = text(&repo.skies(&["proof", "impact", "src/unrelated.txt", "src/nowhere.txt"]));
    assert!(
        by_glob.starts_with("0003-other  no receipt\n  via src/unrelated.txt\n"),
        "{by_glob}"
    );
    assert!(by_glob.contains("1 path in no spec: src/nowhere.txt"), "{by_glob}");

    // A directory reaches every spec whose footprint lies under it; a stale receipt says so.
    repo.write("src/feature.txt", "on\nrefactored\n");
    let by_dir = text(&repo.skies(&["proof", "impact", "src"]));
    assert!(
        by_dir.starts_with("0001-toggle  stale (1 file changed: src/feature.txt)\n  via src\n"),
        "{by_dir}"
    );

    // Without paths, the files changed on the branch (committed, uncommitted, untracked) are the input.
    let by_diff = text(&repo.skies(&["proof", "impact"]));
    assert!(by_diff.contains("0001-toggle  stale"), "{by_diff}");
    assert!(by_diff.contains("0002-twin  no receipt"), "{by_diff}");
    let none = text(&repo.skies(&["proof", "impact", "--diff", "HEAD", "src/nowhere.txt"]));
    assert!(
        none.contains("0001-toggle"),
        "uncommitted edits count against HEAD too: {none}"
    );
}

#[test]
fn record_with_impacted_reproves_overlapping_specs() {
    let repo = two_specs();

    let plain = repo.skies(&["proof", "record", "2"]);
    assert!(plain.status.success(), "{}", text(&plain));
    assert!(text(&plain).contains("impacted: 0001-toggle"), "{}", text(&plain));
    assert!(text(&plain).contains("--with-impacted"));
    assert!(
        repo.json(&format!("{TWIN}/receipt.json"))
            .get("verified_with")
            .is_none()
    );

    let with = repo.skies(&["proof", "record", "2", "--with-impacted"]);
    assert!(with.status.success(), "{}", text(&with));
    assert!(
        text(&with).contains("0001-toggle  verified  2/2 FMs pass"),
        "{}",
        text(&with)
    );
    let receipt = repo.json(&format!("{TWIN}/receipt.json"));
    let toggle_receipt = std::fs::read(repo.path(&format!("{SPEC}/receipt.json"))).unwrap();
    assert_eq!(
        receipt["verified_with"]["0001-toggle"],
        format!("blake3:{}", blake3::hash(&toggle_receipt).to_hex()),
        "names the exact receipt that was re-proven"
    );
}

#[test]
fn an_impacted_spec_that_fails_is_reported_and_left_out() {
    let repo = two_specs();
    repo.write(
        &format!("{SPEC}/e2e/cases.txt"),
        "FM-1: toggles broken\nFM-2: retries once\n",
    );
    let before = repo.read(&format!("{SPEC}/receipt.json"));

    let with = repo.skies(&["proof", "record", "2", "--with-impacted"]);

    assert_eq!(with.status.code(), Some(1), "{}", text(&with));
    let output = text(&with);
    assert!(output.contains("0001-toggle  failed    FM-1 not passing"), "{output}");
    assert!(
        output.contains("0002-twin: recorded, but this change breaks impacted spec 0001-toggle"),
        "{output}"
    );
    let receipt = repo.json(&format!("{TWIN}/receipt.json"));
    assert_eq!(
        receipt["green"]["cases"]["FM-1"], "pass",
        "the new receipt is still written"
    );
    assert!(receipt.get("verified_with").is_none());
    assert_eq!(
        repo.read(&format!("{SPEC}/receipt.json")),
        before,
        "a failed verify leaves its receipt alone"
    );
}

/// A repo whose feature lives in a module with a ctx.md, committed on main so red sees both.
fn module_repo() -> Repo {
    let repo = Repo::new();
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("src/Modules/Toggle/Toggle.cs", "class Toggle {}\n");
    repo.write("src/Modules/Toggle/Toggle.ctx.md", "# toggle\n");
    repo.write("web/src/features/toggle/Toggle.tsx", "export {}\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "--quiet", "-m", "add the module"]);
    repo.git(&["checkout", "--quiet", "-B", "feature"]);
    repo.write(
        &format!("{SPEC}/spec.md"),
        &spec_md("0001", &["FM-1 toggling does nothing"]),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\n");
    repo
}

#[test]
fn impact_names_the_ctx_of_every_module_the_paths_reach() {
    let repo = module_repo();
    let output = text(&repo.skies(&[
        "proof",
        "impact",
        "src/Modules/Toggle/Toggle.cs",
        "web/src/features/toggle/Toggle.tsx",
        "src/unrelated.txt",
    ]));
    assert!(
        output.contains("module context, read before writing failure modes: src/Modules/Toggle/Toggle.ctx.md\n"),
        "{output}"
    );
    assert_eq!(
        output.matches("module context").count(),
        1,
        "a frontend feature has no ctx: {output}"
    );
}

#[test]
fn record_notes_an_unrevised_ctx_and_records_a_revised_one() {
    let repo = module_repo();
    repo.write("src/Modules/Toggle/Toggle.cs", "class Toggle { bool on; }\n");
    repo.implement();

    let unrevised = repo.skies(&["proof", "record", "1"]);
    assert!(
        unrevised.status.success(),
        "a note never fails the record: {}",
        text(&unrevised)
    );
    assert!(
        text(&unrevised).contains(
            "note: Toggle.ctx.md was not revised in this change; update its design notes and cite this spec \
             (`0001-toggle#FM-n`) if an invariant changed."
        ),
        "{}",
        text(&unrevised)
    );
    assert!(repo.json(&format!("{SPEC}/receipt.json")).get("ctx_revised").is_none());

    repo.write(
        "src/Modules/Toggle/Toggle.ctx.md",
        "# toggle\n\nToggling flips once (`0001-toggle#FM-1`).\n",
    );
    let revised = repo.skies(&["proof", "record", "1"]);
    assert!(revised.status.success(), "{}", text(&revised));
    assert!(!text(&revised).contains("was not revised"), "{}", text(&revised));
    let receipt = repo.json(&format!("{SPEC}/receipt.json"));
    assert_eq!(
        receipt["ctx_revised"],
        serde_json::json!(["src/Modules/Toggle/Toggle.ctx.md"])
    );
    assert!(
        receipt["footprint"].get("src/Modules/Toggle/Toggle.ctx.md").is_none(),
        "a ctx is kept fresh by citation, not hashed into the footprint: {receipt}"
    );
    assert!(receipt["footprint"].get("src/Modules/Toggle/Toggle.cs").is_some());
}
