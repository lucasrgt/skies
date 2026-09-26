//! Explicit AVP applicability decisions prevent silent omission. Review attribution is auditable prose, not
//! authentication: the author must obtain a real review; the CLI cannot establish who typed a person's name.

use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::BTreeSet;

use super::report::spec_line;
use super::spec::SpecDoc;

/// Why direct assertions suffice for this mode, and who reviewed that judgment against the AVP catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Exemption {
    pub reason: String,
    pub reviewed_by: String,
}

/// Reads the human-reviewed exceptions after all failure modes are known, irrespective of section order.
pub fn read_exemptions(text: &str, doc: &mut SpecDoc) -> Result<()> {
    let mut active = false;
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            active = heading.trim().eq_ignore_ascii_case("AVP exemptions");
            continue;
        }
        if !active {
            continue;
        }
        let Some((id, value)) = spec_line(line).map_err(anyhow::Error::msg)? else {
            continue;
        };
        if !seen.insert(id) {
            bail!("{id}: duplicate AVP exemption");
        }
        let Some(mode) = doc.modes.get_mut(&id) else {
            bail!("{id}: AVP exemption names an unknown failure mode");
        };
        if !mode.avp_declared || !mode.avp.is_empty() {
            bail!("{id}: AVP exemption requires exactly [avp: none]");
        }
        let Some((reason, reviewer)) = value.split_once("| reviewed-by:") else {
            bail!("{id}: AVP exemption needs a reason and `| reviewed-by: <actual reviewer>`");
        };
        if !substantive(reason) || !substantive(reviewer) {
            bail!(
                "{id}: AVP exemption needs a concrete reason and named reviewer; empty values and placeholders are refused"
            );
        }
        mode.avp_exemption = Some(Exemption {
            reason: reason.trim().into(),
            reviewed_by: reviewer.trim().into(),
        });
    }
    Ok(())
}

/// Refuses incomplete decisions before launching any runner, for both `run` and `record`.
pub fn unprovable(doc: &SpecDoc) -> Option<String> {
    for (id, mode) in &doc.modes {
        if !mode.avp_declared {
            return Some(format!(
                "{id}: missing AVP decision; declare [avp: criterion-id] or [avp: none] with a reviewed AVP exemption"
            ));
        }
        if mode.avp.is_empty() && mode.avp_exemption.is_none() {
            return Some(format!(
                "{id}: [avp: none] needs an AVP exemption under ## AVP exemptions: `- {id} <specific reason> | reviewed-by: <actual reviewer>`"
            ));
        }
    }
    None
}

fn substantive(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.contains(['<', '>'])
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "none" | "nothing" | "n/a" | "na" | "todo" | "tbd" | "pending" | "unknown" | "unreviewed" | "..." | "-"
        )
}
