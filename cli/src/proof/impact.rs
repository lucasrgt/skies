//! `skies proof impact`: which specs a change reaches, read from the module ctx.md citations and `touches`.
//!
//! A path under `**/Modules/<M>/` belongs to `<M>.ctx.md`, whose design notes cite the specs proving the module's
//! invariants (`` `0002-withdraw` ``, `` `0002-withdraw#FM-2` ``; SKY0005 keeps those citations resolvable). A spec
//! may also claim paths with `touches:` globs in its frontmatter. Nothing is hashed and no receipt is read: the
//! answer is what a reader must keep true before writing new failure modes, not a verdict.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use globset::Glob;
use regex::Regex;

use super::base::Base;
use super::git::Repo;
use super::report::FmId;
use super::spec::{self, SpecDoc};
use crate::manifest::Project;

/// Why a spec is impacted, and which of its failure modes to show (`None`: all of them).
#[derive(Default)]
struct Hit {
    via: BTreeSet<String>,
    modes: Option<BTreeSet<FmId>>,
    whole: bool,
}

pub fn impact(paths: &[PathBuf]) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let cwd = std::env::current_dir()?;
    let changed: BTreeSet<String> = if paths.is_empty() {
        let repo = Repo::open(root)?;
        let base = Base::default(&repo, project.manifest.workspace.default_branch.as_deref())?;
        let (line, warning) = base.describe(&repo);
        println!("changes since {line}");
        if let Some(warning) = warning {
            eprintln!("{warning}");
        }
        repo.changed_since(&base.commit)?.into_iter().collect()
    } else {
        paths.iter().map(|path| project_path(root, &cwd, path)).collect()
    };
    if changed.is_empty() {
        println!("no changed files");
        return Ok(0);
    }

    let specs = spec::discover(root)?;
    let mut hits: BTreeMap<String, Hit> = BTreeMap::new();
    let ctxs: BTreeSet<String> = changed.iter().filter_map(|path| module_ctx(path)).collect();
    for ctx in ctxs.iter().filter(|ctx| root.join(ctx).is_file()) {
        let text = std::fs::read_to_string(root.join(ctx)).with_context(|| format!("reading {ctx}"))?;
        let name = ctx.rsplit('/').next().unwrap_or(ctx);
        for (cited, mode) in citations(&text) {
            let hit = hits.entry(cited).or_default();
            hit.via.insert(format!("cited by {name}"));
            match mode {
                Some(id) => {
                    hit.modes.get_or_insert_with(BTreeSet::new).insert(id);
                }
                None => hit.whole = true,
            }
        }
        println!("module context, read before writing failure modes: {ctx}");
    }
    let mut docs = BTreeMap::new();
    for spec in &specs {
        let doc = SpecDoc::load(spec)?;
        for glob in &doc.touches {
            let matcher = Glob::new(glob)
                .with_context(|| format!("{}: invalid touches glob '{glob}'", spec.rel()))?
                .compile_matcher();
            if let Some(path) = changed.iter().find(|path| matcher.is_match(path)) {
                let hit = hits.entry(spec.name.clone()).or_default();
                hit.via.insert(format!("touches {path}"));
                hit.whole = true;
            }
        }
        docs.insert(spec.name.clone(), doc);
    }

    let mut count = 0;
    for (name, hit) in &hits {
        // A citation of a spec that does not exist is SKY0005's finding, not an impact.
        let Some(doc) = docs.get(name) else { continue };
        count += 1;
        println!("{name}  ({})", hit.via.iter().cloned().collect::<Vec<_>>().join(", "));
        for id in &doc.failure_modes {
            if hit.whole || hit.modes.as_ref().is_some_and(|modes| modes.contains(id)) {
                let mode = &doc.modes[id];
                let tag = if mode.avp.is_empty() {
                    String::new()
                } else {
                    format!(" [avp: {}]", mode.avp.join(", "))
                };
                println!("  - {id} {}{tag}", mode.text);
            }
        }
    }
    match count {
        0 => println!("no spec cites or touches these paths"),
        1 => println!("1 spec impacted"),
        count => println!("{count} specs impacted"),
    }
    Ok(0)
}

/// The ctx.md of the module `path` sits in (a file or a folder under `Modules/<M>/`), by convention only; whether it
/// exists is the caller's question. The innermost `Modules/` segment wins.
fn module_ctx(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let at = parts[..parts.len().saturating_sub(1)]
        .iter()
        .rposition(|part| *part == "Modules")?;
    let module = parts[at + 1];
    Some(format!("{}/{module}/{module}.ctx.md", parts[..=at].join("/")))
}

/// `` `<digits>-<slug>` `` or `` `<digits>-<slug>#FM-n` ``, the same citation SKY0005 resolves. The slug must hold a
/// letter, so a backticked date or range is prose.
static CITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([0-9]+-[a-z0-9]+(?:-[a-z0-9]+)*)(?:#FM-([0-9]+))?`").expect("valid regex"));

/// Every spec citation in a ctx: the spec folder and the failure mode, if one is named.
fn citations(text: &str) -> Vec<(String, Option<FmId>)> {
    CITATION
        .captures_iter(text)
        .filter(|capture| {
            let slug = capture[1].split_once('-').map_or("", |(_, slug)| slug);
            slug.chars().any(|ch| ch.is_ascii_lowercase())
        })
        .map(|capture| {
            let mode = capture.get(2).and_then(|n| n.as_str().parse().ok()).map(FmId);
            (capture[1].to_string(), mode)
        })
        .collect()
}

/// A path given on the command line (relative to where the user stands, or absolute), made relative to the project
/// root with forward slashes. Resolved lexically so a file that was deleted still maps to its module.
fn project_path(root: &Path, cwd: &Path, given: &Path) -> String {
    let joined = cwd.join(given);
    let mut parts: Vec<Component> = Vec::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let absolute: PathBuf = parts.iter().collect();
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let absolute = match (absolute.parent().map(std::fs::canonicalize), absolute.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => absolute,
    };
    let rel = absolute.strip_prefix(&root).unwrap_or(&absolute);
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_under_a_module_maps_to_that_modules_ctx() {
        assert_eq!(
            module_ctx("backend/Api/Modules/Wallets/Slices/Deposit.cs").as_deref(),
            Some("backend/Api/Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(
            module_ctx("Modules/Wallets/Wallet.cs").as_deref(),
            Some("Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(
            module_ctx("backend/Api/Modules/Wallets").as_deref(),
            Some("backend/Api/Modules/Wallets/Wallets.ctx.md")
        );
        assert_eq!(module_ctx("backend/Api/Modules"), None);
        assert_eq!(module_ctx("frontend/web/src/features/deposit/Deposit.tsx"), None);
    }

    #[test]
    fn reads_spec_citations_and_ignores_numbers_in_prose() {
        let text = "Overdraw is refused (`0002-withdraw#FM-2`; see `0001-deposit`). Week `2026-31`, `Wallet`.";
        assert_eq!(
            citations(text),
            [
                ("0002-withdraw".to_string(), Some(FmId(2))),
                ("0001-deposit".to_string(), None)
            ]
        );
    }

    #[test]
    fn command_line_paths_become_project_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/a")).unwrap();
        let cwd = root.path().join("src");
        assert_eq!(project_path(root.path(), &cwd, Path::new("a/B.cs")), "src/a/B.cs");
        assert_eq!(project_path(root.path(), &cwd, Path::new("../Other.cs")), "Other.cs");
        assert_eq!(project_path(root.path(), &cwd, &root.path().join("src/a")), "src/a");
    }
}
