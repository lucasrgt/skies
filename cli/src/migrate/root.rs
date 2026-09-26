//! Declares `[workspace] root` in a manifest that has none, from what sits at the root once the migration is done.
//!
//! The list is today's root as git sees it (tracked or untracked, never ignored), minus what the migration deletes
//! and plus what it creates, so a migrated application is doctor-clean on its first run. Junk included: deciding what
//! is junk is the owner's call, so the follow-up names every entry to prune.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{Context, Result};

use super::{Change, Plan};
use crate::doctor::workspace::{self, Entry};
use crate::manifest::{FILE_NAME, Manifest};

pub fn declare(root: &Path, plan: &mut Plan) -> Result<()> {
    let path = root.join(FILE_NAME);
    let planned = plan
        .changes
        .iter()
        .position(|change| matches!(change, Change::Write { path: p, .. } if *p == path));
    let text = match planned {
        Some(index) => match &plan.changes[index] {
            Change::Write { content, .. } => String::from_utf8_lossy(content).into_owned(),
            Change::Delete { .. } => unreachable!("position matched a write"),
        },
        None => std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?,
    };
    let Ok(manifest) = toml::from_str::<Manifest>(&text) else {
        return Ok(());
    };
    if manifest.workspace.root.is_some() {
        return Ok(());
    }

    let entries = entries_after(root, plan);
    let patterns: Vec<String> = entries.iter().map(Entry::pattern).collect();
    let listed: Vec<String> = entries.iter().map(Entry::display).collect();
    let Some(updated) = insert(&text, &patterns) else {
        plan.follow_up(format!(
            "{FILE_NAME}: add a `root` to [workspace] listing what may sit at the repository root (today: {})",
            listed.join(", ")
        ));
        return Ok(());
    };
    match planned {
        Some(index) => {
            plan.changes[index] = Change::Write {
                path,
                content: updated.into_bytes(),
            }
        }
        None => plan.write(&path, updated),
    }
    plan.follow_up(format!(
        "{FILE_NAME}: declared {} root entries in [workspace] root from what is there today ({}); delete the junk \
         and trim the list (`skies doctor` flags anything undeclared as SKYWS001)",
        entries.len(),
        listed.join(", ")
    ));
    Ok(())
}

/// The root entries git sees today, adjusted for what the plan deletes at the root and creates under it.
fn entries_after(root: &Path, plan: &Plan) -> Vec<Entry> {
    let mut entries: BTreeSet<Entry> = workspace::visible_entries(root).into_iter().collect();
    for change in &plan.changes {
        match change {
            Change::Delete { path } if path.parent() == Some(root) => {
                entries.retain(|entry| Some(entry.name.as_str()) != path.file_name().and_then(|name| name.to_str()));
            }
            Change::Write { path, .. } => {
                let Ok(relative) = path.strip_prefix(root) else {
                    continue;
                };
                let mut components = relative.components();
                if let Some(Component::Normal(first)) = components.next() {
                    let name = first.to_string_lossy().into_owned();
                    if name != FILE_NAME && name != ".git" {
                        let is_dir = components.next().is_some();
                        entries.insert(Entry { is_dir, name });
                    }
                }
            }
            Change::Delete { .. } => {}
        }
    }
    let mut out: Vec<Entry> = entries.into_iter().collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Adds `root = [...]` at the end of the `[workspace]` table's keys, keeping every other byte (comments included).
/// `None` when the manifest has no `[workspace]` header or the result would not read back as the same list.
fn insert(text: &str, patterns: &[String]) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.iter().position(|line| is_header(line, Some("workspace")))?;
    let next = lines[header + 1..]
        .iter()
        .position(|line| is_header(line, None))
        .map_or(lines.len(), |offset| header + 1 + offset);
    let mut at = next;
    while at > header + 1 && {
        let line = lines[at - 1].trim();
        line.is_empty() || line.starts_with('#')
    } {
        at -= 1;
    }
    let block = format!(
        "# Everything allowed at the repository root (`skies doctor`: SKYWS001).\n\
         # A trailing / names a folder, no slash a file; globs work.\n{}",
        workspace::render_root(patterns)
    );
    let mut out = String::new();
    for line in &lines[..at] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&block);
    for line in &lines[at..] {
        out.push_str(line);
        out.push('\n');
    }
    let reread = toml::from_str::<Manifest>(&out).ok()?;
    (reread.workspace.root.as_deref() == Some(patterns)).then_some(out)
}

/// A `[table]` header line (not an array-of-tables `[[...]]`), optionally the one named `name`.
fn is_header(line: &str, name: Option<&str>) -> bool {
    let line = line.trim();
    let Some(inner) = line.strip_prefix('[').filter(|rest| !rest.starts_with('[')) else {
        return false;
    };
    let Some(end) = inner.find(']') else {
        return false;
    };
    let rest = inner[end + 1..].trim();
    (rest.is_empty() || rest.starts_with('#')) && name.is_none_or(|name| inner[..end].trim() == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn inserts_root_at_the_end_of_the_workspace_table_keeping_comments() {
        let text = "# top\n[workspace]\nname = \"a\" # the name\n\n# products\n[products.app]\nbackend = \"src/A\"\n";
        let out = insert(text, &["src/".into(), "\".skies/".into()]).unwrap();
        assert!(
            out.starts_with("# top\n[workspace]\nname = \"a\" # the name\n# Everything allowed"),
            "{out}"
        );
        assert!(
            out.ends_with("]\n\n# products\n[products.app]\nbackend = \"src/A\"\n"),
            "{out}"
        );
        let manifest: Manifest = toml::from_str(&out).unwrap();
        assert_eq!(manifest.workspace.root.unwrap(), ["src/", "\".skies/"]);
    }

    #[test]
    fn a_manifest_without_a_workspace_header_is_left_to_a_person() {
        assert!(insert("workspace.name = \"a\"\n", &["src/".into()]).is_none());
    }

    #[test]
    fn declares_what_is_there_after_the_migration() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("Skies.toml"), "[workspace]\nname = \"a\"\n").unwrap();
        fs::write(root.join(".gitignore"), "bin/\n").unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/x.dll"), "").unwrap();
        fs::create_dir_all(root.join("\".skies")).unwrap();
        fs::write(root.join("\".skies/state"), "").unwrap();
        fs::write(root.join("VERIFICATION.md"), "").unwrap();
        let mut plan = Plan::default();
        plan.delete(&root.join("VERIFICATION.md"));
        plan.write(&root.join("tools/helper.sh"), String::new());

        declare(root, &mut plan).unwrap();

        let Some(Change::Write { content, .. }) = plan.changes.last() else {
            panic!("no manifest write: {:?}", plan.changes);
        };
        let manifest: Manifest = toml::from_str(std::str::from_utf8(content).unwrap()).unwrap();
        assert_eq!(manifest.workspace.root.unwrap(), ["\".skies/", "tools/", ".gitignore"]);
        assert!(
            plan.follow_ups
                .keys()
                .any(|note| note.contains("declared 3 root entries"))
        );
    }
}
