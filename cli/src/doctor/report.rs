//! The doctor's output: one summary table, then the findings grouped by leg.

use std::fmt::Write;
use std::path::Path;
use std::time::Duration;

use super::{Leg, Severity, Status};

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
            Status::Ran if leg.findings.is_empty() => None,
            Status::Ran => {
                let mut lines = String::new();
                for finding in &leg.findings {
                    let file = finding
                        .file
                        .strip_prefix(root)
                        .unwrap_or(&finding.file)
                        .display()
                        .to_string();
                    let location = finding.line.map_or(file.clone(), |line| format!("{file}:{line}"));
                    let level = if finding.severity == Severity::Warning {
                        " (warning)"
                    } else {
                        ""
                    };
                    let _ = writeln!(lines, "  {location}  {}{level}  {}", finding.code, finding.message);
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
        Status::Ran => leg.findings.len().to_string(),
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
    use crate::doctor::Finding;
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
                duration: Duration::from_millis(4200),
            },
            Leg {
                name: "flutter app".into(),
                status: Status::Ran,
                findings: vec![],
                duration: Duration::ZERO,
            },
            Leg {
                name: "eslint web".into(),
                status: Status::Failed("`npm` is not on PATH".into()),
                findings: vec![],
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
}
