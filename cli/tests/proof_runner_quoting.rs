//! The .NET runner strings the framework ships, run through the engine's own shell path with a fake `dotnet`: the
//! TRX logger's `trx;LogFileName={report}` holds a `;`, which ends the command inside `sh -c` unless it is quoted, so
//! the report path must reach `dotnet test` intact or the run has no report.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Stands in for `dotnet`: `build` succeeds, `test` writes a passing TRX case to the path its logger argument names.
const FAKE_DOTNET: &str = r#"#!/bin/sh
[ "$1" = test ] || exit 0
report=
for arg in "$@"; do
  case "$arg" in trx\;LogFileName=*) report="${arg#trx;LogFileName=}" ;; esac
done
[ -n "$report" ] || { echo "no trx;LogFileName argument reached dotnet: $*"; exit 1; }
printf '<TestRun><Results><UnitTestResult testName="FM-1: the report path arrived" outcome="Passed"/></Results></TestRun>' > "$report"
"#;

fn repo_file(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

fn run_with_fake_dotnet(manifest: &str) -> (bool, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("Skies.toml"), manifest).unwrap();
    let spec = root.join(".specs/0001-quoting");
    std::fs::create_dir_all(spec.join("e2e")).unwrap();
    std::fs::write(
        spec.join("spec.md"),
        "---\nid: \"0001\"\nrunner: api\n---\n# Quoting\n\n## Failure modes\n\n- FM-1 the report path is cut at `;`\n",
    )
    .unwrap();
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let dotnet = bin.join("dotnet");
    std::fs::write(&dotnet, FAKE_DOTNET).unwrap();
    std::fs::set_permissions(&dotnet, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let output = Command::new(env!("CARGO_BIN_EXE_skies"))
        .args(["proof", "run", "1"])
        .current_dir(root)
        .env("PATH", path)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

#[test]
fn the_shipped_dotnet_runners_hand_the_report_path_to_dotnet_intact() {
    for manifest in ["cli/templates/app/Skies.toml", "examples/sample-app/Skies.toml"] {
        let (passed, text) = run_with_fake_dotnet(&std::fs::read_to_string(repo_file(manifest)).unwrap());
        assert!(passed, "{manifest}: {text}");
        assert!(text.contains("1/1 FMs pass"), "{manifest}: {text}");
    }
    // The harness can tell: unquoted, the shell cuts the command at `;` and dotnet never sees the report path.
    let unquoted =
        "[workspace]\nname = \"x\"\n[runners.api]\ncommand = \"dotnet test --logger trx;LogFileName={report}\"\n";
    let (passed, text) = run_with_fake_dotnet(unquoted);
    assert!(!passed, "{text}");
    assert!(text.contains("no trx;LogFileName argument reached dotnet"), "{text}");
}

#[test]
fn every_documented_trx_logger_is_quoted() {
    for file in [
        "cli/templates/app/Skies.toml",
        "examples/sample-app/Skies.toml",
        "cli/src/manifest.rs",
        "cli/src/proof/mod.rs",
        "docs/CONVENTIONS.md",
        "docs/MONOREPO-ARCHITECTURE.md",
    ] {
        let text = std::fs::read_to_string(repo_file(file)).unwrap();
        for (at, _) in text.match_indices("trx;") {
            let before = text[..at].chars().last();
            assert!(
                matches!(before, Some('\'' | '"')),
                "{file}: `trx;` at byte {at} is not quoted, so `sh -c` ends the command at its `;`"
            );
        }
    }
}

/// Placeholders are quoted for where they sit, so a project, a spec folder, and a temporary folder with spaces in
/// their paths reach the runner as one argument each.
#[test]
fn paths_with_spaces_reach_the_runner_whole() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("my app");
    let tmp = dir.path().join("tmp dir");
    std::fs::create_dir_all(&tmp).unwrap();
    let spec = root.join(".specs/0001-my toggle");
    std::fs::create_dir_all(spec.join("e2e")).unwrap();
    std::fs::write(
        root.join("Skies.toml"),
        "[workspace]\nname = \"x\"\n[runners.fake]\ncommand = \"sh 'run it.sh' {dir} --out={report} \\\"{evidence}\\\"\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("run it.sh"),
        "#!/bin/sh\n[ $# -eq 3 ] || { echo \"expected 3 arguments, got $#: $*\"; exit 1; }\n\
         [ -d \"$3\" ] || { echo \"no evidence folder $3\"; exit 1; }\n\
         name=$(cat \"$1/case.txt\")\nreport=\"${2#--out=}\"\n\
         printf '<testsuite><testcase name=\"%s\"/></testsuite>' \"$name\" > \"$report\"\n",
    )
    .unwrap();
    std::fs::write(
        spec.join("spec.md"),
        "---\nid: \"0001\"\nrunner: fake\n---\n# Toggle\n\n## Failure modes\n\n- FM-1 a path with a space breaks\n",
    )
    .unwrap();
    std::fs::write(spec.join("e2e/case.txt"), "FM-1: spaces survive").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_skies"))
        .args(["proof", "run", "1"])
        .current_dir(&root)
        .env("TMPDIR", &tmp)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{text}");
    assert!(text.contains("1/1 FMs pass"), "{text}");
}
