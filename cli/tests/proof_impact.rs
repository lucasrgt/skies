//! `skies proof impact` through the real binary: the specs a module's ctx.md cites, and the specs whose `touches`
//! match, with their failure modes. No receipt is needed.

mod support;

use support::{Repo, SPEC, spec_md, text};

/// A module with a ctx.md citing one whole spec and one failure mode of another, and a third spec that claims a
/// frontend file through `touches`.
fn cited_repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/Modules/Toggle/Toggle.cs", "class Toggle {}\n");
    repo.write(
        "src/Modules/Toggle/Toggle.ctx.md",
        "# Toggle\n\n## Design notes\n\n- Toggling flips once (`0001-toggle`); a retry never flips twice \
         (`0002-retry#FM-2`). Shipped in week `2026-31`.\n",
    );
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
    repo.write(
        ".specs/0002-retry/spec.md",
        &spec_md("0002", &["FM-1 a retry is refused", "FM-2 a retry flips again"]),
    );
    repo.write(
        ".specs/0003-screen/spec.md",
        "---\nid: \"0003\"\nrunner: fake\ntouches: [web/src/**]\n---\n## Failure modes\n\n\
         - FM-1 the screen shows a stale state, written over\n  two lines\n",
    );
    repo
}

#[test]
fn impact_lists_the_specs_a_modules_ctx_cites() {
    let repo = cited_repo();
    let output = repo.skies(&["proof", "impact", "src/Modules/Toggle/Toggle.cs"]);
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(
        text(&output),
        "module context, read before writing failure modes: src/Modules/Toggle/Toggle.ctx.md\n\
         0001-toggle  (cited by Toggle.ctx.md)\n  - FM-1 toggling does nothing\n  \
         - FM-2 a retry toggles twice [avp: key-honored]\n\
         0002-retry  (cited by Toggle.ctx.md)\n  - FM-2 a retry flips again\n\
         2 specs impacted\n"
    );
}

#[test]
fn impact_lists_the_specs_whose_touches_match() {
    let repo = cited_repo();
    let output = text(&repo.skies(&["proof", "impact", "web/src/Screen.tsx", "src/unrelated.txt"]));
    assert_eq!(
        output,
        "0003-screen  (touches web/src/Screen.tsx)\n  - FM-1 the screen shows a stale state, written over two lines\n\
         1 spec impacted\n"
    );
    let none = text(&repo.skies(&["proof", "impact", "src/unrelated.txt"]));
    assert_eq!(none, "no spec cites or touches these paths\n");
}

#[test]
fn impact_without_paths_reads_the_branch_diff() {
    let repo = cited_repo();
    repo.git(&["add", "."]);
    repo.git(&["commit", "--quiet", "-m", "specs"]);
    repo.write("src/Modules/Toggle/Toggle.cs", "class Toggle { bool on; }\n");
    let output = text(&repo.skies(&["proof", "impact"]));
    assert!(output.starts_with("changes since "), "{output}");
    assert!(output.contains("0001-toggle  (cited by Toggle.ctx.md)"), "{output}");
}
