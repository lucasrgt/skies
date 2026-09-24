//! `.specs/<id>-<slug>/spec.md`: the feature's failure modes, written before the code and read by people.
//!
//! The frontmatter is deliberately a tiny YAML subset (`key: value`, inline or dashed lists) so the file stays
//! hand-editable and the parser stays a few lines instead of a YAML dependency.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::report::{FmId, fm_ids};

pub const SPECS_DIR: &str = ".specs";
pub const SPEC_FILE: &str = "spec.md";
pub const E2E_DIR: &str = "e2e";
pub const RECEIPT_FILE: &str = "receipt.json";
pub const EVIDENCE_DIR: &str = "evidence";
pub const RED_PATCH_FILE: &str = "red.patch";

/// A spec folder on disk, identified by its numeric prefix.
#[derive(Debug, Clone)]
pub struct SpecDir {
    /// The folder name, e.g. `0012-cancel-reservation`.
    pub name: String,
    /// The numeric prefix as written, e.g. `0012`.
    pub id: String,
    /// The folder, absolute.
    pub path: PathBuf,
}

impl SpecDir {
    /// The folder relative to the project root, with forward slashes, as receipts and runners see it.
    pub fn rel(&self) -> String {
        format!("{SPECS_DIR}/{}", self.name)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

/// Every `.specs/<digits>-<slug>/` that holds a `spec.md`, ordered by id. Folders without a spec.md (design
/// notes, archives) are not specs and are skipped silently.
pub fn discover(root: &Path) -> Result<Vec<SpecDir>> {
    let dir = root.join(SPECS_DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut specs = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = numeric_prefix(&name) else { continue };
        if entry.path().join(SPEC_FILE).is_file() {
            specs.push(SpecDir {
                id: id.to_string(),
                name,
                path: entry.path(),
            });
        }
    }
    specs.sort_by(|a, b| (id_number(&a.id), &a.name).cmp(&(id_number(&b.id), &b.name)));
    Ok(specs)
}

/// Finds a spec by folder name, id as written (`0012`), or id as a number (`12`).
pub fn find(root: &Path, key: &str) -> Result<SpecDir> {
    let specs = discover(root)?;
    let wanted = key.trim_end_matches('/').rsplit('/').next().unwrap_or(key);
    let number = wanted.parse::<u64>().ok();
    specs
        .iter()
        .find(|spec| spec.name == wanted || spec.id == wanted || number.is_some_and(|n| id_number(&spec.id) == n))
        .cloned()
        .with_context(|| {
            let known: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
            format!(
                "no spec '{key}' under {SPECS_DIR}/ (found: {})",
                if known.is_empty() {
                    "none".into()
                } else {
                    known.join(", ")
                }
            )
        })
}

fn numeric_prefix(name: &str) -> Option<&str> {
    let (prefix, rest) = name.split_once('-')?;
    (!prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_digit()) && !rest.is_empty()).then_some(prefix)
}

fn id_number(id: &str) -> u64 {
    id.parse().unwrap_or(u64::MAX)
}

/// One `- FM-n text [avp: criterion, …]` line. The tag is optional and never required by the framework: it says
/// that a verifier from the AVP catalog decides this failure mode, so the receipt also demands its verdict.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FailureMode {
    /// The line's text after the id, without the tag.
    pub text: String,
    /// AVP criterion ids from the tag, in the order written.
    pub avp: Vec<String>,
}

/// The parts of spec.md the engine acts on. Everything else in the file is for people.
#[derive(Debug, Default)]
pub struct SpecDoc {
    pub id: Option<String>,
    pub runner: Option<String>,
    pub touches: Vec<String>,
    pub failure_modes: Vec<FmId>,
    /// Each failure mode's line as a reader sees it, and the Assay criteria that must also pass for it.
    pub modes: BTreeMap<FmId, FailureMode>,
    /// Failure modes the `## Non-discriminating` section justifies passing on red.
    pub justified: Vec<FmId>,
}

impl SpecDoc {
    pub fn load(spec: &SpecDir) -> Result<SpecDoc> {
        let path = spec.file(SPEC_FILE);
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let doc = parse(&text).with_context(|| format!("parsing {}", path.display()))?;
        if let Some(id) = &doc.id
            && id_number(id) != id_number(&spec.id)
        {
            bail!(
                "{}: frontmatter id {id} does not match the folder id {}",
                path.display(),
                spec.id
            );
        }
        Ok(doc)
    }

    /// The AVP criteria tagged on a failure mode; empty when it is decided by its cases alone.
    pub fn avp(&self, id: FmId) -> &[String] {
        self.modes.get(&id).map(|mode| mode.avp.as_slice()).unwrap_or_default()
    }

    pub fn runner(&self, spec: &SpecDir) -> Result<&str> {
        self.runner
            .as_deref()
            .with_context(|| format!("{}/{SPEC_FILE} has no `runner:` in its frontmatter", spec.rel()))
    }
}

pub fn parse(text: &str) -> Result<SpecDoc> {
    let mut doc = SpecDoc::default();
    let body = match split_frontmatter(text) {
        Some((front, body)) => {
            parse_frontmatter(front, &mut doc)?;
            body
        }
        None => text,
    };
    let mut section = String::new();
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            section = heading.trim().to_ascii_lowercase();
            continue;
        }
        match section.as_str() {
            "failure modes" => {
                if let Some((id, mode)) = failure_mode_line(line)? {
                    if doc.failure_modes.contains(&id) {
                        bail!("{id} is listed twice under ## Failure modes");
                    }
                    doc.failure_modes.push(id);
                    doc.modes.insert(id, mode);
                }
            }
            "non-discriminating" => doc.justified.extend(fm_ids(line)),
            _ => {}
        }
    }
    Ok(doc)
}

/// `- FM-3 text` (or `* FM-3: text`): the id must lead the bullet, so prose that merely mentions a failure mode
/// inside the section does not declare one.
fn failure_mode_line(line: &str) -> Result<Option<(FmId, FailureMode)>> {
    let Some(item) = line.trim_start().strip_prefix(['-', '*']).map(str::trim_start) else {
        return Ok(None);
    };
    let Some(head) = item.split_whitespace().next() else {
        return Ok(None);
    };
    let id = match fm_ids(head).as_slice() {
        [id] if head.to_ascii_uppercase().starts_with("FM") => *id,
        _ => return Ok(None),
    };
    let rest = item[head.len()..].trim_start_matches(':').trim();
    let (text, avp) = avp_tag(rest).with_context(|| format!("{id}: malformed [avp: …] tag"))?;
    Ok(Some((id, FailureMode { text, avp })))
}

/// Splits `text [avp: a, b]` into the text and the criterion ids. A criterion id is kebab-case, as in the AVP
/// catalog, so a typo like `[avp: ]` or a missing bracket is an error instead of a silently untagged mode.
fn avp_tag(rest: &str) -> Result<(String, Vec<String>)> {
    let Some(start) = rest.to_ascii_lowercase().find("[avp:") else {
        return Ok((rest.to_string(), Vec::new()));
    };
    let end = rest[start..]
        .find(']')
        .map(|offset| start + offset)
        .context("the tag has no closing ]")?;
    let ids: Vec<String> = rest[start + "[avp:".len()..end]
        .split(',')
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    if ids.is_empty() {
        bail!("the tag names no criterion (expected e.g. [avp: idempotency-key-honored])");
    }
    if let Some(bad) = ids.iter().find(|id| validate_slug(id).is_err()) {
        bail!("'{bad}' is not an AVP criterion id (kebab-case, e.g. idempotency-key-honored)");
    }
    let text = format!("{} {}", rest[..start].trim_end(), rest[end + 1..].trim_start());
    Ok((text.trim().to_string(), ids))
}

fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim_start_matches('\u{feff}').strip_prefix("---")?;
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn parse_frontmatter(front: &str, doc: &mut SpecDoc) -> Result<()> {
    let mut list_key: Option<String> = None;
    for line in front.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            match list_key.as_deref() {
                Some("touches") => doc.touches.push(unquote(item).to_string()),
                Some(_) => {}
                None => bail!("frontmatter list item '{trimmed}' has no key above it"),
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            bail!("frontmatter line '{trimmed}' is not `key: value`");
        };
        let (key, value) = (key.trim(), strip_comment(value).trim());
        list_key = value.is_empty().then(|| key.to_string());
        match key {
            "id" => doc.id = Some(unquote(value).to_string()),
            "runner" => doc.runner = Some(unquote(value).to_string()),
            "touches" if !value.is_empty() => doc.touches = inline_list(value)?,
            _ => {}
        }
    }
    Ok(())
}

fn inline_list(value: &str) -> Result<Vec<String>> {
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .with_context(|| format!("expected a list like [a/**, b.cs], found '{value}'"))?;
    Ok(inner
        .split(',')
        .map(|item| unquote(item.trim()).to_string())
        .filter(|item| !item.is_empty())
        .collect())
}

/// Drops a trailing `# comment` that sits outside quotes, as YAML would.
fn strip_comment(value: &str) -> &str {
    let mut quote = None;
    for (index, ch) in value.char_indices() {
        match (quote, ch) {
            (None, '"' | '\'') => quote = Some(ch),
            (Some(open), _) if ch == open => quote = None,
            (None, '#') if index == 0 || value[..index].ends_with(char::is_whitespace) => return &value[..index],
            _ => {}
        }
    }
    value
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(quote).and_then(|rest| rest.strip_suffix(quote)) {
            return inner;
        }
    }
    value
}

/// The next id: one past the largest numeric prefix under `.specs/`, counting folders without a spec.md too so
/// design-only folders never have their number reused.
pub fn next_id(root: &Path) -> Result<String> {
    let dir = root.join(SPECS_DIR);
    let mut max = 0;
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if let Some(prefix) = numeric_prefix(&name) {
                max = max.max(id_number(prefix));
            }
        }
    }
    Ok(format!("{:04}", max + 1))
}

pub fn validate_slug(slug: &str) -> Result<()> {
    let valid = !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid {
        bail!(
            "slug '{slug}' must be kebab-case: lowercase letters, digits, and single hyphens (e.g. cancel-reservation)"
        );
    }
    Ok(())
}

pub fn template(id: &str, slug: &str, runner: &str) -> String {
    let title = slug.replace('-', " ");
    let title = title[..1].to_ascii_uppercase() + &title[1..];
    format!(
        "---\n\
         id: \"{id}\"\n\
         runner: {runner}\n\
         # touches: [src/Module/**]   # optional: files the receipt should also watch\n\
         ---\n\
         # {title}\n\
         \n\
         What the feature does, for whom, and why, in a few sentences.\n\
         \n\
         ## Failure modes\n\
         \n\
         <!-- One line per way the feature can fail. Each becomes an e2e case titled \"FM-n: ...\" that fails before\n\
         the implementation and passes after it. -->\n\
         \n\
         - FM-1 ...\n\
         \n\
         ## Out of scope\n\
         \n\
         - ...\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nid: \"0012\"   # the folder id\nrunner: web\ntouches: [src/Res/**, 'src/Pay.cs']\n---\n# Cancel\n\nCancel mentions FM-9 in prose.\n\n## Failure modes\n\n- FM-1 other guest -> 404\n- FM-2: twice -> one refund\n* FM-3 after check-in -> 409\nSee FM-7 in passing.\n\n## Non-discriminating\n\n- FM-2 the refund ledger already dedupes at the base revision.\n\n## Out of scope\n- FM-8 is not a failure mode here\n";

    #[test]
    fn reads_frontmatter_and_failure_modes() {
        let doc = parse(SAMPLE).unwrap();
        assert_eq!(doc.id.as_deref(), Some("0012"));
        assert_eq!(doc.runner.as_deref(), Some("web"));
        assert_eq!(doc.touches, ["src/Res/**", "src/Pay.cs"]);
        assert_eq!(doc.failure_modes, [FmId(1), FmId(2), FmId(3)]);
        assert_eq!(doc.justified, [FmId(2)]);
    }

    #[test]
    fn reads_avp_tags_and_keeps_them_out_of_the_text() {
        let doc = parse(
            "## Failure modes\n- FM-1 plain\n- FM-5: a retry credits twice [avp: idempotency-key-honored]\n\
             - FM-6 two tags [AVP: a-b, c] trailing\n",
        )
        .unwrap();
        assert!(doc.modes[&FmId(1)].avp.is_empty());
        assert_eq!(doc.modes[&FmId(1)].text, "plain");
        assert_eq!(doc.modes[&FmId(5)].text, "a retry credits twice");
        assert_eq!(doc.modes[&FmId(5)].avp, ["idempotency-key-honored"]);
        assert_eq!(doc.modes[&FmId(6)].avp, ["a-b", "c"]);
        assert_eq!(doc.modes[&FmId(6)].text, "two tags trailing");
    }

    #[test]
    fn rejects_malformed_avp_tags() {
        for line in ["- FM-1 x [avp: ]", "- FM-1 x [avp: Not Kebab]", "- FM-1 x [avp: a"] {
            let error = parse(&format!("## Failure modes\n{line}\n")).unwrap_err();
            assert!(format!("{error:#}").contains("FM-1"), "{line}: {error:#}");
        }
    }

    #[test]
    fn reads_dashed_touches() {
        let doc = parse("---\nid: 1\nrunner: api\ntouches:\n  - a/**\n  - \"b.cs\"\n---\n").unwrap();
        assert_eq!(doc.touches, ["a/**", "b.cs"]);
    }

    #[test]
    fn rejects_duplicate_failure_modes() {
        let error = parse("## Failure modes\n- FM-1 a\n- FM-1 b\n").unwrap_err();
        assert!(error.to_string().contains("FM-1 is listed twice"));
    }

    #[test]
    fn the_template_parses_back() {
        let doc = parse(&template("0004", "cancel-reservation", "api")).unwrap();
        assert_eq!(doc.id.as_deref(), Some("0004"));
        assert_eq!(doc.runner.as_deref(), Some("api"));
        assert!(doc.touches.is_empty());
        assert_eq!(doc.failure_modes, [FmId(1)]);
        assert!(template("0004", "cancel-reservation", "api").contains("# Cancel reservation"));
    }

    #[test]
    fn numbers_and_finds_specs() {
        let root = tempfile::tempdir().unwrap();
        for name in ["0001-a", "0009-notes", "0010-b"] {
            std::fs::create_dir_all(root.path().join(SPECS_DIR).join(name)).unwrap();
        }
        for name in ["0001-a", "0010-b"] {
            std::fs::write(root.path().join(SPECS_DIR).join(name).join(SPEC_FILE), "").unwrap();
        }
        assert_eq!(next_id(root.path()).unwrap(), "0011");
        let names: Vec<String> = discover(root.path())
            .unwrap()
            .into_iter()
            .map(|spec| spec.name)
            .collect();
        assert_eq!(names, ["0001-a", "0010-b"]);
        assert_eq!(find(root.path(), "10").unwrap().name, "0010-b");
        assert_eq!(find(root.path(), "0001-a").unwrap().id, "0001");
        assert!(find(root.path(), "0009").is_err());
    }

    #[test]
    fn validates_slugs() {
        assert!(validate_slug("cancel-reservation").is_ok());
        assert!(validate_slug("Cancel").is_err());
        assert!(validate_slug("-x").is_err());
        assert!(validate_slug("a b").is_err());
    }
}
