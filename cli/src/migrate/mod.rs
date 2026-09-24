//! `skies migrate 5`: moves a Skies 4 application onto Skies 5.
//!
//! The migration removes what Skies 5 no longer has (proof annotations and tags, spec manifests, flow contracts,
//! verification reports, CSM state, the foundations block, gate hooks, the dotnet tool) and rewrites `Skies.toml`
//! to the 5.x schema. It never deletes a test: existing tests keep running as ordinary tests. Everything it cannot
//! decide safely is reported as a follow-up instead of guessed.

mod eslint;
mod manifest;
mod package_json;
mod scripts;
mod source;
mod vendor;
mod versions;
mod workflows;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

/// Directories never scanned: build output, dependencies, and tool caches.
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "bin",
    "obj",
    "build",
    "dist",
    "target",
    ".dart_tool",
    ".gradle",
    "Pods",
    ".expo",
    ".next",
    ".turbo",
    "coverage",
    ".claude",
];

/// One file-level effect of the migration.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The new bytes of a file; text edits are UTF-8 unless the file was in a legacy encoding.
    Write {
        path: PathBuf,
        content: Vec<u8>,
    },
    Delete {
        path: PathBuf,
    },
}

/// Everything the migration will do, plus what a person has to finish by hand.
#[derive(Debug, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
    /// Notes for a person, each with the files it applies to (empty for repository-wide notes).
    pub follow_ups: BTreeMap<String, Vec<String>>,
    /// Helper copies already scheduled, so a helper used by many files is written once per package.
    pub vendored: BTreeSet<PathBuf>,
    /// Whether the dotnet tool manifest goes away with `skies-framework-cli`, which makes `dotnet tool restore` fail.
    pub tool_manifest_removed: bool,
    /// Packages that import a helper copy from another package instead of receiving their own (see
    /// `vendor::shared_homes`).
    pub shared_homes: BTreeMap<PathBuf, PathBuf>,
}

impl Plan {
    fn write(&mut self, path: &Path, content: String) {
        self.write_bytes(path, content.into_bytes());
    }

    fn write_bytes(&mut self, path: &Path, content: Vec<u8>) {
        self.changes.push(Change::Write {
            path: path.to_path_buf(),
            content,
        });
    }

    fn delete(&mut self, path: &Path) {
        self.changes.push(Change::Delete {
            path: path.to_path_buf(),
        });
    }

    fn follow_up(&mut self, note: impl Into<String>) {
        self.follow_ups.entry(note.into()).or_default();
    }

    fn follow_up_file(&mut self, note: &str, file: &str) {
        self.follow_ups
            .entry(note.to_string())
            .or_default()
            .push(file.to_string());
    }
}

pub fn run(version: u32, dry_run: bool) -> Result<u8> {
    if version != 5 {
        bail!("only `skies migrate 5` is supported");
    }
    let root = std::env::current_dir()?;
    if !root.join(crate::manifest::FILE_NAME).is_file() {
        bail!("run from the repository root (no {} here)", crate::manifest::FILE_NAME);
    }
    let plan = plan(&root)?;
    print(&root, &plan, dry_run);
    if !dry_run {
        apply(&plan)?;
    }
    Ok(0)
}

/// Computes the full migration without touching the disk.
pub fn plan(root: &Path) -> Result<Plan> {
    let mut plan = Plan::default();
    manifest::migrate(root, &mut plan)?;

    for name in ["VERIFICATION.md", "VERIFICATION.json"] {
        let path = root.join(name);
        if path.is_file() {
            plan.delete(&path);
        }
    }
    for dir in [".skies/verification-attempt", ".skies/foundation"] {
        let path = root.join(dir);
        if path.is_dir() {
            plan.delete(&path);
        }
    }
    // The CSM stores are the team's recorded decisions, patterns, scars, and deferments. Skies 5 stops running the
    // tools, but the knowledge is the repository's, so it stays where it is.
    if root.join("csm.toml").is_file() || root.join(".skies/csm").is_dir() {
        plan.follow_up(
            "csm.toml and .skies/csm are kept: Skies no longer runs the CSM tools (why-this-way, right-this-way, \
             not-you-again, now-we-can); install them on their own to keep using the records, or delete both",
        );
    }

    plan.tool_manifest_removed = [".config/dotnet-tools.json", "dotnet-tools.json"]
        .iter()
        .any(|manifest| {
            std::fs::read_to_string(root.join(manifest))
                .ok()
                .and_then(|text| source::dotnet_tools(&text, manifest, &mut Plan::default()))
                .is_some_and(|rest| rest.is_empty())
        });

    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !skipped(entry.path(), root))
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    plan.shared_homes = vendor::shared_homes(root, &files);
    for path in &files {
        source::migrate_file(root, path, &mut plan).with_context(|| format!("migrating {}", path.display()))?;
    }
    Ok(plan)
}

/// Build output, dependencies, and hidden tool directories (caches, agent workspaces) are never application
/// source; `.config` and `.github` are the exceptions that can carry Skies 4 wiring.
fn skipped(path: &Path, root: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path != root
        && (SKIPPED_DIRS.contains(&name)
            || (name.starts_with('.') && path.is_dir() && ![".config", ".github"].contains(&name)))
}

/// Writes the report in one go and ignores a closed stdout, so `skies migrate 5 --dry-run | head` never panics.
fn print(root: &Path, plan: &Plan, dry_run: bool) {
    use std::fmt::Write as _;
    let mut out = String::new();
    let verb = if dry_run { "would" } else { "will" };
    let relative = |path: &Path| path.strip_prefix(root).unwrap_or(path).display().to_string();
    let (mut writes, mut deletes) = (0, 0);
    for change in &plan.changes {
        match change {
            Change::Write { path, .. } => {
                writes += 1;
                writeln!(out, "  edit    {}", relative(path)).unwrap();
            }
            Change::Delete { path } => {
                deletes += 1;
                writeln!(out, "  delete  {}", relative(path)).unwrap();
            }
        }
    }
    writeln!(
        out,
        "skies migrate 5 {verb} edit {writes} and delete {deletes} path(s)."
    )
    .unwrap();
    if !plan.follow_ups.is_empty() {
        writeln!(out, "\nFinish by hand:").unwrap();
        for (note, files) in &plan.follow_ups {
            match files.as_slice() {
                [] => writeln!(out, "  - {note}").unwrap(),
                [one] => writeln!(out, "  - {one}: {note}").unwrap(),
                many => {
                    writeln!(out, "  - {note} ({} files):", many.len()).unwrap();
                    for file in many.iter().take(5) {
                        writeln!(out, "      {file}").unwrap();
                    }
                    if many.len() > 5 {
                        writeln!(out, "      … and {} more", many.len() - 5).unwrap();
                    }
                }
            }
        }
    }
    let _ = std::io::Write::write_all(&mut std::io::stdout(), out.as_bytes());
}

fn apply(plan: &Plan) -> Result<()> {
    for change in &plan.changes {
        match change {
            Change::Write { path, content } => {
                // Vendored helpers land in folders the application may not have yet.
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
                }
                fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
            }
            Change::Delete { path } if path.is_dir() => {
                fs::remove_dir_all(path).with_context(|| format!("deleting {}", path.display()))?;
            }
            Change::Delete { path } => {
                fs::remove_file(path).with_context(|| format!("deleting {}", path.display()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn migrates_a_skies_4_repository() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "Skies.toml",
            "[workspace]\nname = \"demo\"\n\n[products.app]\nbackend = \"src/Demo.Api\"\ncore = \"clients/core\"\nfrontend = \"clients/web\"\n\n[framework]\nrepo = \"../skies\"\n",
        );
        write(root, "csm.toml", "");
        write(root, ".skies/csm/lock.toml", "");
        write(root, "VERIFICATION.md", "");
        write(
            root,
            "src/Demo.Api/Modules/Billing/Billing.spec.toml",
            "module = \"Billing\"\n[slices.Pay]\ncriteria = [\"x\"]\n",
        );
        write(
            root,
            "src/Demo.Api/Journeys/PayJourney.Tests.cs",
            "public class PayJourney\n{\n    [E2E]\n    [Journey(typeof(Pay), JourneyPath.Happy)]\n    [Fact]\n    public void Pays() { }\n\n    [Unit, Fact]\n    public void Quick() { }\n}\n",
        );
        write(
            root,
            "clients/web/e2e/flows.json",
            "[{\"id\":\"pay-happy\",\"criteria\":[]}]",
        );
        write(
            root,
            "clients/web/src/pay/Pay.viewModel.ts",
            "/**\n * Pays an invoice.\n * @verify pays-once\n * @e2e pay-happy\n */\nexport function usePayModel() {}\n",
        );

        let plan = plan(root).unwrap();
        apply(&plan).unwrap();

        let manifest = fs::read_to_string(root.join("Skies.toml")).unwrap();
        assert!(
            manifest.contains("frontend = [\"clients/core\", \"clients/web\"]"),
            "{manifest}"
        );
        assert!(!manifest.contains("[framework]"));
        assert!(root.join("csm.toml").exists(), "team knowledge is never deleted");
        assert!(root.join(".skies/csm/lock.toml").exists());
        assert!(!root.join("VERIFICATION.md").exists());
        assert!(!root.join("src/Demo.Api/Modules/Billing/Billing.spec.toml").exists());
        assert!(!root.join("clients/web/e2e/flows.json").exists());

        let journey = fs::read_to_string(root.join("src/Demo.Api/Journeys/PayJourney.Tests.cs")).unwrap();
        assert_eq!(
            journey,
            "public class PayJourney\n{\n    [Fact]\n    public void Pays() { }\n\n    [Fact]\n    public void Quick() { }\n}\n"
        );
        let view_model = fs::read_to_string(root.join("clients/web/src/pay/Pay.viewModel.ts")).unwrap();
        assert_eq!(
            view_model,
            "/**\n * Pays an invoice.\n */\nexport function usePayModel() {}\n"
        );
    }

    #[test]
    fn assay_proofs_keep_their_tags_and_only_lose_removed_imports() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "Skies.toml", "[workspace]\nname = \"demo\"\n");
        write(root, "clients/web/package.json", "{}\n");
        write(
            root,
            "clients/web/src/pay/Pay.assay.test.tsx",
            "import { productVerification } from \"@skiesjs/frontend-sdk/product-verification\";\n/** @avp pays-once */\n",
        );
        write(root, "clients/app/pubspec.yaml", "name: app\n");
        write(
            root,
            "clients/app/test/pay.assay_test.dart",
            "void main() {\n  // @avp pays-once\n}\n",
        );

        apply(&plan(root).unwrap()).unwrap();

        let web = fs::read_to_string(root.join("clients/web/src/pay/Pay.assay.test.tsx")).unwrap();
        assert!(web.contains("/** @avp pays-once */"), "{web}");
        assert!(!web.contains("@skiesjs/frontend-sdk"), "{web}");
        let dart = fs::read_to_string(root.join("clients/app/test/pay.assay_test.dart")).unwrap();
        assert!(dart.contains("// @avp pays-once"));
    }

    #[test]
    fn a_migrated_repository_needs_no_second_pass() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Skies.toml",
            "[workspace]\nname = \"demo\"\n\n[products.app]\nbackend = \"src/Demo.Api\"\n",
        );
        let plan = plan(dir.path()).unwrap();
        assert!(plan.changes.is_empty(), "{:?}", plan.changes);
    }
}
