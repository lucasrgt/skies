//! `skies migrate 5`: moves a Skies 4 application onto Skies 5.
//!
//! The migration removes what Skies 5 no longer has (proof annotations and tags, spec manifests, flow contracts,
//! verification reports, CSM state, the foundations block, gate hooks, the dotnet tool) and rewrites `Skies.toml`
//! to the 5.x schema. It never deletes a test: existing tests keep running as ordinary tests. Everything it cannot
//! decide safely is reported as a follow-up instead of guessed.

mod eslint;
mod imports;
mod manifest;
mod package_json;
mod root;
mod scripts;
mod source;
mod versions;
mod workflows;

use std::collections::BTreeMap;
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
    /// Whether the dotnet tool manifest goes away with `skies-framework-cli`, which makes `dotnet tool restore` fail.
    pub tool_manifest_removed: bool,
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
    for path in &files {
        source::migrate_file(root, path, &mut plan).with_context(|| format!("migrating {}", path.display()))?;
        note_generated_auth(root, path, &mut plan);
    }
    // Last, so the declared root reflects what the other changes delete and create.
    root::declare(root, &mut plan)?;
    Ok(plan)
}

/// The follow-up for an Account module generated by Skies 4's `g auth`.
const GENERATED_AUTH_NOTE: &str = "auth mechanics now live in Skies.Framework.Auth; regenerate or port with \
                                   `skies g auth` into a branch and compare";

/// Skies 4's `g auth` wrote the auth mechanics (hashing, refresh rotation, revocation, token checks) into the app;
/// Skies 5 ships them in `Skies.Framework.Auth`. That code is the app's own now and may carry its changes, so it is
/// never rewritten, only pointed at. The tell is a `Refresh` slice that does not delegate to `RefreshSessions`.
fn note_generated_auth(root: &Path, path: &Path, plan: &mut Plan) {
    if !path.ends_with("Modules/Account/Slices/Refresh.cs") {
        return;
    }
    if fs::read_to_string(path).is_ok_and(|text| text.contains("RefreshSessions")) {
        return;
    }
    let relative = path.strip_prefix(root).unwrap_or(path).display().to_string();
    plan.follow_up_file(GENERATED_AUTH_NOTE, &relative);
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
    fn assay_proofs_are_left_alone_and_reported() {
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

        let plan = plan(root).unwrap();
        assert!(plan.follow_ups.keys().any(|note| note.contains("Skies 4 frontend SDK")));
        apply(&plan).unwrap();

        let web = fs::read_to_string(root.join("clients/web/src/pay/Pay.assay.test.tsx")).unwrap();
        assert!(web.contains("/** @avp pays-once */"), "{web}");
        assert!(web.contains("@skiesjs/frontend-sdk/product-verification"), "{web}");
        let dart = fs::read_to_string(root.join("clients/app/test/pay.assay_test.dart")).unwrap();
        assert!(dart.contains("// @avp pays-once"));
    }

    #[test]
    fn a_generated_skies_4_auth_module_is_pointed_at_never_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "Skies.toml", "[workspace]\nname = \"demo\"\n");
        let refresh = "src/Demo.Api/Modules/Account/Slices/Refresh.cs";
        let skies_4 =
            "public static class Refresh\n{\n    internal static Task RevokeFamily() => Task.CompletedTask;\n}\n";
        write(root, refresh, skies_4);
        write(
            root,
            "src/Ported.Api/Modules/Account/Slices/Refresh.cs",
            "// sessions.RotateAsync via RefreshSessions\n",
        );

        let plan = plan(root).unwrap();

        let files = plan
            .follow_ups
            .iter()
            .find(|(note, _)| note.contains("Skies.Framework.Auth"))
            .map(|(_, files)| files.clone())
            .expect("the generated auth module gets a follow-up");
        assert_eq!(files, [refresh.replace('/', std::path::MAIN_SEPARATOR_STR)]);
        assert!(
            !plan
                .changes
                .iter()
                .any(|change| matches!(change, Change::Write { path, .. } if path.ends_with("Refresh.cs"))),
            "the app's auth code is never rewritten"
        );
    }

    #[test]
    fn a_migrated_repository_needs_no_second_pass() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Skies.toml",
            "[workspace]\nname = \"demo\"\n\n[products.app]\nbackend = \"src/Demo.Api\"\n",
        );
        write(dir.path(), "src/Demo.Api/Program.cs", "");
        write(dir.path(), "VERIFICATION.md", "");
        let first = plan(dir.path()).unwrap();
        apply(&first).unwrap();

        let manifest = crate::manifest::load(&dir.path().join("Skies.toml")).unwrap();
        let declared = manifest.workspace.root.expect("migrate declares the root");
        assert_eq!(declared, ["src/"]);
        let (_, findings) = crate::doctor::workspace::check(dir.path(), Some(&declared));
        assert!(findings.is_empty(), "a migrated root is doctor-clean: {findings:?}");
        let second = plan(dir.path()).unwrap();
        assert!(second.changes.is_empty(), "{:?}", second.changes);
    }
}
