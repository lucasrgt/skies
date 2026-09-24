//! The everyday loop through the real binary: `proof run` (judge the cases now, commit nothing), a baseline
//! `verify` that leaves current receipts alone, a runner `scope` that keeps a surface's receipts to its own files,
//! and failure modes written over several lines.

mod support;

use std::process::Command;

use support::{Repo, SPEC, new_spec, spec_md, text};

fn porcelain(repo: &Repo) -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repo.path(""))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn run_judges_each_failure_mode_and_commits_nothing() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: always answers\n");

    let before = repo.skies(&["proof", "run", "1"]);
    assert_eq!(before.status.code(), Some(1), "{}", text(&before));
    let output = text(&before);
    assert!(output.contains("  FM-1  fail  toggling does nothing\n"), "{output}");
    assert!(output.contains("failed: FM-1: toggles"), "{output}");
    assert!(output.contains("  FM-2  pass  toggling twice breaks\n"), "{output}");
    assert!(output.contains("1 of 2 FMs fail"), "{output}");
    let committed = |repo: &Repo| -> Vec<String> {
        let evidence = repo.path(&format!("{SPEC}/evidence"));
        let mut names: Vec<String> = std::fs::read_dir(&evidence)
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    assert_eq!(committed(&repo), ["raw"], "run writes no committed evidence");
    assert!(
        repo.read(&format!("{SPEC}/evidence/raw/run.xml"))
            .contains("FM-1: toggles")
    );
    assert!(repo.path(&format!("{SPEC}/evidence/raw/run.log")).is_file());
    assert!(!repo.path(&format!("{SPEC}/receipt.json")).exists());
    repo.git(&["add", ".specs"]);
    assert!(
        !porcelain(&repo).contains("evidence/raw"),
        "the local report is gitignored: {}",
        porcelain(&repo)
    );

    repo.implement();
    let after = repo.skies(&["proof", "run", "0001-toggle"]);
    assert!(after.status.success(), "{}", text(&after));
    assert!(text(&after).contains("2/2 FMs pass"), "{}", text(&after));
    assert_eq!(committed(&repo), ["raw"]);
}

#[test]
fn run_shows_what_a_failing_case_reported() {
    let repo = Repo::with_runner(|runner| {
        runner.replace(
            r#"echo "<testcase name=\"$name\"><failure/></testcase>"; fi"#,
            r#"echo "<testcase name=\"$name\"><failure message=\"expected on, got $state\"/></testcase>"; fi"#,
        )
    });
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    let output = text(&repo.skies(&["proof", "run", "1"]));
    assert!(output.contains("  | expected on, got off"), "{output}");
}

#[test]
fn a_baseline_verify_of_current_receipts_changes_nothing() {
    let repo = Repo::new();
    new_spec(&repo, "FM-1: toggles\nFM-2: toggles twice\n");
    repo.implement();
    let recorded = repo.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    repo.git(&["add", ".specs"]);
    repo.git(&["commit", "--quiet", "-m", "receipt"]);
    assert_eq!(porcelain(&repo), "");

    let baseline = repo.skies(&["proof", "verify", "1"]);
    assert!(baseline.status.success(), "{}", text(&baseline));
    assert!(
        text(&baseline).contains("0001-toggle  verified (current, unchanged)  2/2 FMs pass"),
        "{}",
        text(&baseline)
    );
    assert_eq!(porcelain(&repo), "", "the receipt and its evidence are untouched");

    let refreshed = repo.skies(&["proof", "verify", "1", "--refresh"]);
    assert!(refreshed.status.success(), "{}", text(&refreshed));
    assert!(
        text(&refreshed).contains("verified  2/2 FMs pass (refreshed;"),
        "{}",
        text(&refreshed)
    );

    repo.write("src/feature.txt", "on\nrefactored\n");
    let stale = repo.skies(&["proof", "verify", "--stale"]);
    assert!(
        text(&stale).contains("verified  2/2 FMs pass (refreshed;"),
        "{}",
        text(&stale)
    );
    assert_eq!(text(&repo.skies(&["proof", "status"])), "0001-toggle  current\n");
}

/// A backend module with a ctx.md and a screen, both on main, and a feature that edits both plus the flag.
fn two_surfaces(scope: Option<&str>) -> Repo {
    let repo = Repo::new();
    repo.git(&["checkout", "--quiet", "main"]);
    let scope = scope.map_or(String::new(), |scope| format!("scope = [{scope}]\n"));
    repo.write(
        "Skies.toml",
        &format!(
            "[workspace]\nname = \"demo\"\n\n[runners.fake]\n{scope}command = \"sh run.sh {{dir}} {{report}} {{evidence}}\"\n"
        ),
    );
    repo.write("backend/Modules/Toggle/Toggle.cs", "class Toggle {}\n");
    repo.write("backend/Modules/Toggle/Toggle.ctx.md", "# toggle\n");
    repo.write("frontend/Toggle.tsx", "export {}\n");
    repo.write("frontend/Shared.ts", "export {}\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "--quiet", "-m", "two surfaces"]);
    repo.git(&["checkout", "--quiet", "-B", "feature"]);
    repo.write(
        &format!("{SPEC}/spec.md"),
        &spec_md("0001", &["FM-1 toggling does nothing"]).replace(
            "runner: fake\n",
            "runner: fake\ntouches: [frontend/Shared.ts, backend/Modules/Toggle/Toggle.cs]\n",
        ),
    );
    repo.write(&format!("{SPEC}/e2e/cases.txt"), "FM-1: toggles\n");
    repo.write("backend/Modules/Toggle/Toggle.cs", "class Toggle { bool on; }\n");
    repo.write("frontend/Toggle.tsx", "export const on = true;\n");
    repo.implement();
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
fn a_runner_scope_keeps_a_receipt_to_its_surface() {
    let scoped = two_surfaces(Some("\"frontend/\""));
    let recorded = scoped.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert_eq!(
        footprint(&scoped),
        ["frontend/Shared.ts", "frontend/Toggle.tsx"],
        "the diff part and touches, inside the scope only"
    );
    assert!(!text(&recorded).contains("was not revised"), "{}", text(&recorded));
    scoped.write("backend/Modules/Toggle/Toggle.cs", "class Toggle { bool off; }\n");
    assert_eq!(text(&scoped.skies(&["proof", "status"])), "0001-toggle  current\n");
    scoped.write("frontend/Shared.ts", "export const shared = 1;\n");
    assert_eq!(
        text(&scoped.skies(&["proof", "status"])),
        "0001-toggle  stale (1 file changed: frontend/Shared.ts)\n"
    );

    let unscoped = two_surfaces(None);
    let recorded = unscoped.skies(&["proof", "record", "1"]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert_eq!(
        footprint(&unscoped),
        [
            "backend/Modules/Toggle/Toggle.cs",
            "frontend/Shared.ts",
            "frontend/Toggle.tsx",
            "src/feature.txt"
        ],
        "without a scope, every changed file counts, as before"
    );
    assert!(
        text(&recorded).contains("note: Toggle.ctx.md was not revised"),
        "{}",
        text(&recorded)
    );
}

#[test]
fn impact_prints_a_failure_mode_written_over_several_lines_whole() {
    let repo = Repo::new();
    repo.write(
        &format!("{SPEC}/spec.md"),
        "---\nid: \"0001\"\nrunner: fake\ntouches: [src/feature.txt]\n---\n## Failure modes\n\n\
         - FM-1 `ErrorBody.code` does not enumerate the app's error codes (the Wallets and\n  \
         Money registries, the platform's rate limit), so the client cannot type them.\n- FM-2 short\n",
    );
    let output = text(&repo.skies(&["proof", "impact", "src/feature.txt"]));
    assert!(
        output.contains(
            "  - FM-1 `ErrorBody.code` does not enumerate the app's error codes (the Wallets and Money registries, \
             the platform's rate limit), so the client cannot type them.\n  - FM-2 short\n"
        ),
        "{output}"
    );
}
