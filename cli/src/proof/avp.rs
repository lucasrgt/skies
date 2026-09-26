//! Assay (AVP) verdicts as spec evidence.
//!
//! A failure mode tagged `[avp: criterion]` in spec.md is decided by a verifier from the AVP catalog as well as by
//! its E2E case. The case saves the verdict to `$SKIES_EVIDENCE/avp-FM-<n>.json`; the engine only reads that file.
//! It does not run Assay, link it, or know its archetypes, so an explicitly reviewed exemption needs no verifier dependency.
//! Applicability is validated before a runner starts; this module only checks the declared criteria.
//!
//! Both Assay implementations are accepted as they serialize themselves: Assay.Net's `Verdict` through
//! System.Text.Json (PascalCase or camelCase, `status` as a string or as the enum number where 0 is Pass) and the
//! TypeScript `verdictToJsonLine` (one NDJSON line, `status: "pass"`). A file may hold one verdict, an array of
//! verdicts, or one verdict per line, since a case may run several archetypes.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use super::report::FmId;
use super::spec::SpecDoc;

/// The verdict file a case proving `id` writes, inside the evidence directory.
pub fn verdict_file(id: FmId) -> String {
    format!("avp-{id}.json")
}

/// Why a tagged failure mode is not proven by its verdict: the message a person reads to fix it.
pub fn check(doc: &SpecDoc, id: FmId, evidence: &Path) -> Result<(), String> {
    let Some(mode) = doc.modes.get(&id).filter(|mode| !mode.avp.is_empty()) else {
        return Ok(());
    };
    let name = verdict_file(id);
    let path = evidence.join(&name);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Err(format!(
            "{id} [avp: {}] has no verdict: its case must save the Assay verdict to $SKIES_EVIDENCE/{name}",
            mode.avp.join(", ")
        ));
    };
    let statuses =
        criterion_statuses(&text).map_err(|error| format!("{id}: {name} is not an Assay verdict: {error}"))?;
    let mut problems = Vec::new();
    for criterion in &mode.avp {
        match statuses.get(criterion.as_str()) {
            None => problems.push(format!("{criterion} is not in the verdict")),
            Some(found) if found.iter().all(|status| status == "pass") => {}
            Some(found) => problems.push(format!("{criterion} is {}", found.join("/"))),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{id} verdict {name}: {}", problems.join("; ")))
    }
}

/// criterion id → every status the file reports for it, lowercased and kebab-cased (`pass`, `not-applicable`).
/// A criterion counts as passing only when every verdict that decided it says pass.
pub fn criterion_statuses(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let verdicts: Vec<Value> = match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(items)) => items,
        Ok(value) => vec![value],
        Err(_) => text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(|error| format!("line is not JSON ({error})")))
            .collect::<Result<_, _>>()?,
    };
    let mut statuses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for verdict in &verdicts {
        let results = field(verdict, "results")
            .and_then(Value::as_array)
            .ok_or("a verdict has no results array")?;
        for result in results {
            let criterion = field(result, "criterionId")
                .and_then(Value::as_str)
                .ok_or("a result has no criterionId")?;
            let status = field(result, "status").ok_or("a result has no status")?;
            statuses
                .entry(criterion.to_string())
                .or_default()
                .push(status_name(status)?);
        }
    }
    Ok(statuses)
}

/// A property by its camelCase name, or the PascalCase System.Text.Json writes by default.
fn field<'a>(value: &'a Value, camel: &str) -> Option<&'a Value> {
    let object = value.as_object()?;
    object.get(camel).or_else(|| {
        let mut pascal = camel.to_string();
        pascal[..1].make_ascii_uppercase();
        object.get(&pascal)
    })
}

/// Normalizes `Pass`, `pass`, `NotApplicable`, `not-applicable`, and the enum numbers of Assay.Net's
/// `VerdictStatus` (Pass, Fail, NotApplicable, Unresolved) to one spelling.
fn status_name(status: &Value) -> Result<String, String> {
    const BY_NUMBER: [&str; 4] = ["pass", "fail", "not-applicable", "unresolved"];
    match status {
        Value::String(text) if !text.chars().any(|ch| ch.is_ascii_lowercase()) => {
            Ok(text.to_ascii_lowercase().replace('_', "-"))
        }
        Value::String(text) => {
            let mut name = String::new();
            for (index, ch) in text.chars().enumerate() {
                if ch.is_ascii_uppercase() && index > 0 {
                    name.push('-');
                }
                name.push(ch.to_ascii_lowercase());
            }
            Ok(name.replace('_', "-"))
        }
        Value::Number(number) => number
            .as_u64()
            .and_then(|index| BY_NUMBER.get(index as usize))
            .map(|name| name.to_string())
            .ok_or_else(|| format!("unknown status {number}")),
        other => Err(format!("unknown status {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::spec;

    const CRITERION: &str = "idempotency-key-honored";

    fn statuses(text: &str) -> Vec<String> {
        criterion_statuses(text).unwrap()[CRITERION].clone()
    }

    #[test]
    fn reads_every_serialization_of_both_implementations() {
        // Assay.Net through System.Text.Json defaults: PascalCase, numeric enum.
        assert_eq!(
            statuses(r#"{"Subject":"Deposit","Results":[{"CriterionId":"idempotency-key-honored","Status":0}]}"#),
            ["pass"]
        );
        // Assay.Net with JsonSerializerDefaults.Web and JsonStringEnumConverter.
        assert_eq!(
            statuses(r#"{"results":[{"criterionId":"idempotency-key-honored","status":"Pass"}],"outcome":"Pass"}"#),
            ["pass"]
        );
        // The TypeScript verdictToJsonLine, one line per verdict.
        let lines = format!(
            "{}\n{}\n",
            r#"{"subject":"a","results":[{"criterionId":"idempotency-key-honored","reason":"r","status":"pass"}]}"#,
            r#"{"subject":"b","results":[{"criterionId":"other","reason":"r","status":"not-applicable"}]}"#
        );
        assert_eq!(statuses(&lines), ["pass"]);
        assert_eq!(criterion_statuses(&lines).unwrap()["other"], ["not-applicable"]);
        assert_eq!(
            statuses(r#"[{"results":[{"criterionId":"idempotency-key-honored","status":2}]}]"#),
            ["not-applicable"]
        );
    }

    #[test]
    fn rejects_what_is_not_a_verdict() {
        assert!(criterion_statuses(r#"{"passed":true}"#).is_err());
        assert!(criterion_statuses("not json").is_err());
        assert!(criterion_statuses(r#"{"results":[{"criterionId":"x","status":9}]}"#).is_err());
    }

    #[test]
    fn a_tagged_mode_needs_a_passing_verdict_for_every_criterion() {
        let doc =
            spec::parse("## Failure modes\n- FM-1 plain\n- FM-2 retried [avp: idempotency-key-honored, b]\n").unwrap();
        let dir = tempfile::tempdir().unwrap();
        assert!(check(&doc, FmId(1), dir.path()).is_ok(), "untagged modes need nothing");

        let missing = check(&doc, FmId(2), dir.path()).unwrap_err();
        assert!(missing.contains("$SKIES_EVIDENCE/avp-FM-2.json"), "{missing}");

        let write = |text: &str| std::fs::write(dir.path().join("avp-FM-2.json"), text).unwrap();
        write(r#"{"results":[{"criterionId":"idempotency-key-honored","status":"pass"}]}"#);
        assert!(
            check(&doc, FmId(2), dir.path())
                .unwrap_err()
                .contains("b is not in the verdict")
        );

        write(
            r#"{"results":[{"criterionId":"idempotency-key-honored","status":"fail"},{"criterionId":"b","status":0}]}"#,
        );
        assert!(
            check(&doc, FmId(2), dir.path())
                .unwrap_err()
                .contains("idempotency-key-honored is fail")
        );

        write(
            r#"{"results":[{"criterionId":"idempotency-key-honored","status":"Pass"},{"criterionId":"b","status":0}]}"#,
        );
        assert!(check(&doc, FmId(2), dir.path()).is_ok());
    }
}
