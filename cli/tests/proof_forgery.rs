//! The ways a receipt could be forged or mislead, each refused through the real binary: a spec folder that shares its
//! id with another, and a failure mode still reading as the template's placeholder.

mod support;

use support::{Repo, SPEC, new_spec, spec_md, text};

#[test]
fn two_specs_sharing_an_id_are_refused_everywhere_and_spec_new_takes_the_next() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.write(".specs/0001-home/spec.md", &spec_md("0001", &["FM-1 the home is empty"]));
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
