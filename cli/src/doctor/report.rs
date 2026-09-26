//! The doctor's output: one summary table, then the findings grouped by leg, each leg's suppressions (with their
//! reasons) after its findings, so an escape hatch is never silent.

use std::fmt::Write;
use std::path::Path;
use std::time::Duration;

use super::{Finding, Leg, Severity, Status, Suppressed};

pub fn render(root: &Path, legs: &[Leg], total: Duration) -> String {
    let mut out = String::new();
    let rows: Vec<[String; 4]> = legs
        .iter()
        .map(|leg| [leg.name.clone(), status(leg), count(leg), seconds(leg.duration)])
        .collect();
    let header = ["leg", "status", "findings", "time"].map(String::from);
    let widths: Vec<usize> = (0..4)
        .map(|i| {
            rows.iter()
                .chain([&header])
                .map(|row| row[i].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in std::iter::once(&header).chain(&rows) {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        let _ = writeln!(out, "{}", cells.join("  ").trim_end());
    }
    let _ = writeln!(out, "total {}", seconds(total));

    for leg in legs {
        let detail = match &leg.status {
            Status::Failed(reason) => Some(indent(reason)),
            Status::Skipped(_) => None,
            Status::Ran if leg.findings.is_empty() && leg.suppressed.is_empty() => None,
            Status::Ran => {
                let mut lines = String::new();
                for finding in &leg.findings {
                    let level = if finding.severity == Severity::Warning {
                        " (warning)"
                    } else {
                        ""
                    };
                    let location = location(root, finding);
                    let _ = writeln!(lines, "  {location}  {}{level}  {}", finding.code, finding.message);
                }
                for Suppressed { finding, reason } in &leg.suppressed {
                    let location = location(root, finding);
                    let _ = writeln!(lines, "  {location}  {} (suppressed)  {reason}", finding.code);
                }
                Some(lines)
            }
        };
        if let Some(detail) = detail {
            let _ = write!(out, "\n{}\n{detail}", leg.name);
        }
    }
    out
}

fn location(root: &Path, finding: &Finding) -> String {
    let file = finding
        .file
        .strip_prefix(root)
        .unwrap_or(&finding.file)
        .display()
        .to_string();
    finding.line.map_or(file.clone(), |line| format!("{file}:{line}"))
}

fn status(leg: &Leg) -> String {
    match &leg.status {
        Status::Failed(_) => "could not run".into(),
        Status::Skipped(reason) => format!("skipped: {reason}"),
        Status::Ran if leg.errors() > 0 => "findings".into(),
        Status::Ran if !leg.findings.is_empty() => "warnings".into(),
        Status::Ran => "clean".into(),
    }
}

fn count(leg: &Leg) -> String {
    match leg.status {
        Status::Ran if leg.suppressed.is_empty() => leg.findings.len().to_string(),
        Status::Ran => format!("{} +{} suppressed", leg.findings.len(), leg.suppressed.len()),
        _ => "-".into(),
    }
}

fn seconds(duration: Duration) -> String {
    format!("{:.1}s", duration.as_secs_f64())
}

fn indent(text: &str) -> String {
    text.lines().map(|line| format!("  {line}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn prints_a_table_then_findings_grouped_by_leg() {
        let legs = vec![
            Leg {
                name: "dotnet api".into(),
                status: Status::Ran,
                findings: vec![Finding::new(
                    "SKY0001",
                    Severity::Error,
                    PathBuf::from("/repo/api/Ping.cs"),
                    Some(3),
                    "Ping must be static".into(),
                )],
                suppressed: vec![],
                duration: Duration::from_millis(4200),
            },
            Leg {
                name: "flutter app".into(),
                status: Status::Ran,
                findings: vec![],
                suppressed: vec![],
                duration: Duration::ZERO,
            },
            Leg {
                name: "eslint web".into(),
                status: Status::Failed("`npm` is not on PATH".into()),
                findings: vec![],
                suppressed: vec![],
                duration: Duration::ZERO,
            },
        ];

        let out = render(Path::new("/repo"), &legs, Duration::from_secs(5));

        assert!(out.starts_with("leg          status         findings  time\n"));
        assert!(out.contains("dotnet api   findings       1         4.2s\n"));
        assert!(out.contains("flutter app  clean          0         0.0s\n"));
        assert!(out.contains("eslint web   could not run  -         0.0s\n"));
        assert!(out.contains("\ndotnet api\n  api/Ping.cs:3  SKY0001  Ping must be static\n"));
        assert!(out.contains("\neslint web\n  `npm` is not on PATH\n"));
        assert!(!out.contains("\nflutter app\n"));
    }

    #[test]
    fn a_suppression_is_printed_with_its_reason_and_never_counted_as_a_finding() {
        let finding = Finding::new(
            "SKYFL029",
            Severity::Error,
            PathBuf::from("/repo/app/lib/legacy.dart"),
            Some(7),
            "refresh rotation is consumed outside the session/client seam".into(),
        );
        let legs = vec![Leg {
            name: "flutter app".into(),
            status: Status::Ran,
            findings: vec![],
            suppressed: vec![Suppressed {
                finding,
                reason: "the legacy client rotates on its own until the port".into(),
            }],
            duration: Duration::ZERO,
        }];

        let out = render(Path::new("/repo"), &legs, Duration::ZERO);

        assert!(out.contains("flutter app  clean   0 +1 suppressed"), "{out}");
        assert!(out.contains(
            "\nflutter app\n  app/lib/legacy.dart:7  SKYFL029 (suppressed)  the legacy client rotates on its own until \
             the port\n"
        ));
        assert_eq!(crate::doctor::exit_code(&legs), 0);
    }
}
