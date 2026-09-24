//! The three leg runners and the parsers that turn their output into findings.
//!
//! Output is captured, never streamed: a build log is noise once its diagnostics are extracted. When a tool fails
//! without producing any diagnostic, the tail of its output becomes the leg's failure reason instead, so a broken
//! restore or a missing SDK is still explained.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Instant;

use regex::Regex;

use super::{Finding, Leg, Severity, Status, Target};

pub fn run(root: &Path, target: &Target, build_args: &[String]) -> Leg {
    let started = Instant::now();
    let relative = |path: &Path| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let (name, (status, findings)) = match target {
        Target::Dotnet(path) => (
            format!("dotnet {}", relative(path)),
            dotnet(path, build_args),
        ),
        Target::Eslint(path) => (format!("eslint {}", relative(path)), eslint(path)),
        Target::Flutter(path) => (format!("flutter {}", relative(path)), flutter(path)),
        Target::Unknown(path) => (
            format!("frontend {}", relative(path)),
            (
                Status::Failed("neither package.json nor pubspec.yaml found".into()),
                Vec::new(),
            ),
        ),
    };
    Leg {
        name,
        status,
        findings,
        duration: started.elapsed(),
    }
}

type Outcome = (Status, Vec<Finding>);

fn dotnet(path: &Path, build_args: &[String]) -> Outcome {
    let mut command = Command::new("dotnet");
    command
        .arg("build")
        .arg(path)
        .args(["-nologo", "-tl:off", "-clp:NoSummary"])
        .args(build_args);
    command.env("DOTNET_CLI_UI_LANGUAGE", "en");
    let Some((success, output)) = capture(command) else {
        return (Status::Failed("`dotnet` is not on PATH".into()), Vec::new());
    };
    let findings = parse_msbuild(&output);
    verdict(success, output, findings)
}

fn eslint(path: &Path) -> Outcome {
    let manifest = std::fs::read_to_string(path.join("package.json")).unwrap_or_default();
    let has_lint = serde_json::from_str::<serde_json::Value>(&manifest)
        .ok()
        .and_then(|json| json.get("scripts")?.get("lint").cloned())
        .is_some();
    if !has_lint {
        return (
            Status::Skipped("no `lint` script in package.json".into()),
            Vec::new(),
        );
    }
    let mut command = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
    command.args(["run", "--silent", "lint"]).current_dir(path);
    let Some((success, output)) = capture(command) else {
        return (Status::Failed("`npm` is not on PATH".into()), Vec::new());
    };
    let findings = parse_stylish(&output);
    verdict(success, output, findings)
}

fn flutter(path: &Path) -> Outcome {
    match crate::flutter::rules::diagnose(path) {
        Ok(findings) => (Status::Ran, findings),
        Err(error) => (Status::Failed(format!("{error:#}")), Vec::new()),
    }
}

/// Runs a command to completion, returning success and the interleaved stdout+stderr text. `None` means the
/// program could not be started at all.
fn capture(mut command: Command) -> Option<(bool, String)> {
    let output = command.output().ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), text))
}

/// A failed tool with no error diagnostics could not produce a verdict; anything else is a verdict.
fn verdict(success: bool, output: String, findings: Vec<Finding>) -> Outcome {
    if !success && !findings.iter().any(|f| f.severity == Severity::Error) {
        let tail: Vec<&str> = output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .rev()
            .take(5)
            .collect();
        let reason = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        return (
            Status::Failed(if reason.is_empty() {
                "exited without diagnostics".into()
            } else {
                reason
            }),
            findings,
        );
    }
    (Status::Ran, findings)
}

static MSBUILD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?P<file>.+?)(?:\((?P<line>\d+)(?:,\d+)*\))?\s*:\s*(?P<severity>warning|error)\s+(?P<code>[A-Z][A-Z0-9]*\d+)\s*:\s*(?P<message>.*?)(?:\s+\[[^\]]+\])?\s*$",
    )
    .expect("static pattern")
});

/// Extracts every SKY diagnostic plus every error (a compile error hides analyzer results, so it must show).
/// MSBuild repeats diagnostics per target framework and in summaries; duplicates collapse.
pub fn parse_msbuild(output: &str) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for captures in output.lines().filter_map(|line| MSBUILD.captures(line)) {
        let code = &captures["code"];
        let severity = if &captures["severity"] == "error" {
            Severity::Error
        } else {
            Severity::Warning
        };
        if !code.starts_with("SKY") && severity == Severity::Warning {
            continue;
        }
        let finding = Finding::new(
            code,
            severity,
            PathBuf::from(captures["file"].trim()),
            captures.name("line").and_then(|l| l.as_str().parse().ok()),
            captures["message"].to_string(),
        );
        if seen.insert(finding.clone()) {
            findings.push(finding);
        }
    }
    findings
}

static STYLISH_ROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(?P<line>\d+):\d+\s+(?P<severity>error|warning)\s+(?P<message>.+?)(?:\s{2,}(?P<rule>\S+))?\s*$")
        .expect("static pattern")
});

/// Parses ESLint's default (stylish) output: a file path line followed by indented `line:col severity message
/// rule` rows. The package's own `lint` script chooses the command, so its default format is what arrives.
pub fn parse_stylish(output: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut file: Option<PathBuf> = None;
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            file = (line.contains('/') || line.contains('\\')).then(|| PathBuf::from(line.trim()));
            continue;
        }
        let (Some(current), Some(row)) = (&file, STYLISH_ROW.captures(line)) else {
            continue;
        };
        let severity = if &row["severity"] == "error" {
            Severity::Error
        } else {
            Severity::Warning
        };
        let code = row.name("rule").map_or("eslint", |r| r.as_str());
        let line_number = row["line"].parse().ok();
        findings.push(Finding::new(
            code,
            severity,
            current.clone(),
            line_number,
            row["message"].to_string(),
        ));
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msbuild_keeps_sky_diagnostics_and_errors_once() {
        let output = "\
/src/Api/Modules/Wallets/Deposit.cs(12,5): warning SKY0026: Wallet has no concurrency token [/src/Api/Api.csproj]
/src/Api/Modules/Wallets/Deposit.cs(12,5): warning SKY0026: Wallet has no concurrency token [/src/Api/Api.csproj]
/src/Api/Program.cs(3,1): warning CS8618: Non-nullable property [/src/Api/Api.csproj]
/src/Api/Program.cs(9,14): error CS0103: The name 'x' does not exist in the current context [/src/Api/Api.csproj]
/src/Api/Slices/Ping.cs(1,1): error SKY0001: Ping must be static [/src/Api/Api.csproj]
CSC : error CS2001: Source file 'gone.cs' could not be found. [/src/Api/Api.csproj]
Build succeeded.";

        let findings = parse_msbuild(output);

        let codes: Vec<&str> = findings.iter().map(|f| f.code.as_str()).collect();
        assert_eq!(codes, ["SKY0026", "CS0103", "SKY0001", "CS2001"]);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].line, Some(12));
        assert_eq!(findings[0].message, "Wallet has no concurrency token");
        assert_eq!(findings[3].line, None);
    }

    #[test]
    fn stylish_rows_attach_to_the_file_above_them() {
        let output = "\
/app/src/items/Items.view.tsx
  4:10  error    SKYFE001 View imports the client  skies/view-purity
  9:3   warning  Unexpected console statement      no-console

/app/src/other.ts
  1:1  error  Parsing error: Unexpected token

✖ 3 problems (2 errors, 1 warning)
";
        let findings = parse_stylish(output);

        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].code, "skies/view-purity");
        assert_eq!(findings[0].message, "SKYFE001 View imports the client");
        assert_eq!(
            findings[0].file,
            PathBuf::from("/app/src/items/Items.view.tsx")
        );
        assert_eq!(findings[1].severity, Severity::Warning);
        assert_eq!(findings[2].code, "eslint");
        assert_eq!(findings[2].line, Some(1));
    }

    #[test]
    fn a_failed_tool_without_diagnostics_is_a_leg_that_could_not_run() {
        let (status, _) = verdict(
            false,
            "restore failed\nNU1101: package not found\n".into(),
            Vec::new(),
        );
        assert_eq!(
            status,
            Status::Failed("restore failed\nNU1101: package not found".into())
        );
        let (status, _) = verdict(true, String::new(), Vec::new());
        assert_eq!(status, Status::Ran);
    }
}
