//! `skies g client` for a React web package: the hand-owned client seams plus an orval run.
//!
//! orval generates the hooks. What it cannot generate are the files every hook calls through: the mutator (auth
//! injection, the injectable base URL, the `X-Client: web` header that turns on the cookie session), the
//! QueryClient with the write-side defaults, the feedback seam, and the orval config itself. Those are written
//! once and then owned by the app; orval's output is regenerated on every run.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use minijinja::context;

use super::contract;
use super::scaffold::{relative_path, render, run, tree_hash, write_missing};

const MUTATOR: &str = include_str!("../../templates/react/client/skies-client.ts");
const FEEDBACK: &str = include_str!("../../templates/react/client/feedback.ts");
const QUERY: &str = include_str!("../../templates/react/client/query.ts");
const ORVAL_CONFIG: &str = include_str!("../../templates/react/client/orval.config.ts");

/// The hand-owned files the app boots through (the mutator, the feedback seam, the QueryClient), relative to the
/// package root. `skies g web-app` writes them up front, so a new package runs before it has a contract.
pub fn render_boot_seams() -> Vec<(&'static str, String)> {
    vec![
        ("src/lib/skies-client.ts", MUTATOR.to_string()),
        ("src/lib/feedback.ts", FEEDBACK.to_string()),
        ("src/lib/query.ts", QUERY.to_string()),
    ]
}

/// The hand-owned files, relative to the package root: the boot seams and the orval config.
pub fn render_seams(name: &str, contract: &str) -> Result<Vec<(&'static str, String)>> {
    let mut seams = render_boot_seams();
    seams.push(("orval.config.ts", render(ORVAL_CONFIG, context! { name, contract })?));
    Ok(seams)
}

pub fn generate(package: &Path) -> Result<u8> {
    // Preflight: everything that can refuse runs before the first write, so a failed run leaves the package as it
    // found it.
    let contract = contract::for_package(package)?;
    let orval = orval_bin(package).with_context(|| {
        format!(
            "orval is not installed for {}; run `npm install --save-dev orval` there (nothing was written)",
            package.display()
        )
    })?;
    let generated = package.join("src/client.gen");
    let hand_written = hand_written(&generated);
    if !hand_written.is_empty() {
        let names: Vec<String> = hand_written.iter().map(|path| relative_path(package, path)).collect();
        bail!(
            "src/client.gen/ is generated output, but it holds hand-written files: {}. Move them out of \
             src/client.gen/ (nothing was written)",
            names.join(", ")
        );
    }

    let contract_path = relative_path(package, &contract.path);
    let seams: Vec<(PathBuf, String)> = render_seams(&contract.name, &contract_path)?
        .into_iter()
        .map(|(file, contents)| (package.join(file), contents))
        .collect();
    write_missing(&seams)?;

    let before = tree_hash(&generated);
    let stale = regenerate(&generated, || {
        run(&orval.to_string_lossy(), &["--config", "orval.config.ts"], package)
    })?;
    for path in &stale {
        println!("removed {} (no longer in the contract)", relative_path(package, path));
    }
    let after = tree_hash(&generated);
    let verdict = if before == after { "unchanged" } else { "updated" };
    println!("client.gen {verdict} from {}", contract.path.display());
    Ok(0)
}

/// Runs `generate` over a `client.gen/` emptied of its generated files, and returns the ones it did not write again.
///
/// orval runs with `clean: false` and skips a file whose content would not change, so neither its output nor file
/// times tell a dropped operation from an unchanged one. The generated files are set aside first (hand-written
/// files never reach this point); whatever the run does not write back is stale and stays deleted. A failed run
/// puts every set-aside file back, so the client is never left half-generated.
fn regenerate(generated: &Path, generate: impl FnOnce() -> Result<()>) -> Result<Vec<PathBuf>> {
    let previous = generated_files(generated);
    if previous.is_empty() {
        generate()?;
        return Ok(Vec::new());
    }
    let parent = generated.parent().unwrap_or(generated);
    let aside = tempfile::Builder::new()
        .prefix(".client.gen-")
        .tempdir_in(parent)
        .with_context(|| format!("creating a scratch folder in {}", parent.display()))?;
    let moved: Vec<(PathBuf, PathBuf)> = previous
        .iter()
        .enumerate()
        .map(|(index, path)| (path.clone(), aside.path().join(index.to_string())))
        .collect();
    for (path, kept) in &moved {
        std::fs::rename(path, kept).with_context(|| format!("setting {} aside", path.display()))?;
    }
    if let Err(error) = generate() {
        for (path, kept) in &moved {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::rename(kept, path);
        }
        return Err(error);
    }
    Ok(previous.into_iter().filter(|path| !path.exists()).collect())
}

/// The orval executable for `package`: its own `node_modules/.bin`, else the nearest ancestor's (a workspace that
/// hoists its dev dependencies), the way npm resolves a package's binaries.
fn orval_bin(package: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "orval.cmd" } else { "orval" };
    package
        .ancestors()
        .map(|dir| dir.join("node_modules/.bin").join(name))
        .find(|path| path.is_file())
}

/// The mark orval puts at the top of every file it writes.
const GENERATED_MARK: &str = "Generated by orval";

/// Whether a file under `client.gen/` carries orval's mark in its header.
fn is_generated(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let head = &bytes[..bytes.len().min(512)];
    String::from_utf8_lossy(head).contains(GENERATED_MARK)
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect();
    out.sort();
    out
}

/// Every file under `dir` that orval wrote.
fn generated_files(dir: &Path) -> Vec<PathBuf> {
    files_under(dir).into_iter().filter(|path| is_generated(path)).collect()
}

/// Every file under `dir` without orval's mark: a file `skies g client` must neither overwrite nor delete.
fn hand_written(dir: &Path) -> Vec<PathBuf> {
    files_under(dir)
        .into_iter()
        .filter(|path| !is_generated(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seam(name: &str) -> String {
        render_seams("shop", "./contract/Shop.Api.json")
            .unwrap()
            .into_iter()
            .find(|(f, _)| *f == name)
            .unwrap()
            .1
    }

    #[test]
    fn the_mutator_injects_the_base_url_at_boot_and_carries_the_auth_seams() {
        let mutator = seam("src/lib/skies-client.ts");
        assert!(mutator.contains("instance.defaults.baseURL ="));
        assert!(
            !regex::Regex::new(r"(?s)axios\.create\(\{[^}]*baseURL")
                .unwrap()
                .is_match(&mutator)
        );
        assert!(mutator.contains("export function configureClient"));
        assert!(mutator.contains("export function setAccessToken"));
        assert!(mutator.contains("Authorization: `Bearer ${accessToken}`"));
        assert!(mutator.contains("\"X-Client\": \"web\""));
        assert!(mutator.contains("withCredentials: true"));
    }

    #[test]
    fn the_mutator_delegates_rotation_to_the_injected_seam() {
        let mutator = seam("src/lib/skies-client.ts");
        assert!(mutator.contains("export function setTokenRefresher"));
        assert!(mutator.contains("await refreshSession()"));
        assert!(!mutator.contains("refreshAccessToken"));
        assert!(mutator.contains("_retried"));
        assert!(mutator.contains("isAuthRoute"));
    }

    #[test]
    fn the_orval_config_wires_the_mutator_and_leaves_writes_as_mutations() {
        let config = seam("orval.config.ts");
        assert!(config.contains("target: \"./contract/Shop.Api.json\""));
        assert!(config.contains("tags: [\"skies:asset\", \"skies:webhook\", \"skies:internal\"]"));
        assert!(config.contains("mutator: { path: \"./src/lib/skies-client.ts\", name: \"skiesClient\" }"));
        assert!(config.contains("target: \"./src/client.gen/shop.ts\""));
        assert!(!regex::Regex::new(r"query:\s*\{").unwrap().is_match(&config));
        assert!(!config.contains("useQuery") && !config.contains("useMutation"));
        assert!(
            config.contains("clean: false,"),
            "generation never deletes the folder it writes into"
        );
    }

    const ORVAL_HEADER: &str = "/**\n * Generated by orval v7.10.0\n * Do not edit manually.\n */\n";

    fn package_with_contract() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Skies.toml"),
            "[workspace]\nname = \"s\"\n[products.app]\nbackend = \"api/Shop.Api\"\nfrontend = \"web\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("api/Shop.Api/contract")).unwrap();
        std::fs::write(dir.path().join("api/Shop.Api/contract/Shop.Api.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("web/src")).unwrap();
        std::fs::write(dir.path().join("web/package.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn a_missing_orval_refuses_before_writing_anything() {
        let dir = package_with_contract();
        let web = dir.path().join("web").canonicalize().unwrap();

        let error = format!("{:#}", generate(&web).unwrap_err());

        assert!(
            error.contains("orval is not installed") && error.contains("nothing was written"),
            "{error}"
        );
        assert!(!web.join("src/lib").exists() && !web.join("orval.config.ts").exists());
    }

    #[test]
    fn a_hand_written_file_in_client_gen_is_refused_not_overwritten() {
        let dir = package_with_contract();
        let web = dir.path().join("web").canonicalize().unwrap();
        std::fs::create_dir_all(web.join("node_modules/.bin")).unwrap();
        std::fs::write(web.join("node_modules/.bin/orval"), "").unwrap();
        std::fs::create_dir_all(web.join("src/client.gen/model")).unwrap();
        std::fs::write(web.join("src/client.gen/shop.ts"), "export const mine = 1;\n").unwrap();
        std::fs::write(web.join("src/client.gen/model/index.ts"), ORVAL_HEADER).unwrap();

        let error = format!("{:#}", generate(&web).unwrap_err());

        assert!(
            error.contains("./src/client.gen/shop.ts") && !error.contains("index.ts"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(web.join("src/client.gen/shop.ts")).unwrap(),
            "export const mine = 1;\n"
        );
        assert!(!web.join("orval.config.ts").exists());
    }

    #[test]
    fn only_files_carrying_orvals_mark_count_as_generated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("model")).unwrap();
        std::fs::write(dir.path().join("shop.ts"), format!("{ORVAL_HEADER}export {{}};\n")).unwrap();
        std::fs::write(dir.path().join("model/item.ts"), ORVAL_HEADER).unwrap();
        std::fs::write(dir.path().join("notes.ts"), "// mine\n").unwrap();

        assert_eq!(
            generated_files(dir.path()),
            [dir.path().join("model/item.ts"), dir.path().join("shop.ts")]
        );
        assert_eq!(hand_written(dir.path()), [dir.path().join("notes.ts")]);
        assert!(generated_files(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn regeneration_removes_only_what_the_run_did_not_write_back() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join("src/client.gen");
        std::fs::create_dir_all(generated.join("model")).unwrap();
        for file in ["shop.ts", "model/item.ts", "model/gone.ts"] {
            std::fs::write(generated.join(file), ORVAL_HEADER).unwrap();
        }

        // orval skips unchanged files; here it "writes" two of three, the way a dropped schema looks.
        let stale = regenerate(&generated, || {
            std::fs::write(generated.join("shop.ts"), ORVAL_HEADER)?;
            std::fs::write(generated.join("model/item.ts"), ORVAL_HEADER)?;
            Ok(())
        })
        .unwrap();

        assert_eq!(stale, [generated.join("model/gone.ts")]);
        assert!(generated.join("shop.ts").is_file() && !generated.join("model/gone.ts").exists());
        assert_eq!(
            std::fs::read_dir(dir.path().join("src")).unwrap().count(),
            1,
            "no scratch folder left"
        );
    }

    #[test]
    fn a_failed_regeneration_restores_the_previous_client() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join("client.gen");
        std::fs::create_dir_all(generated.join("model")).unwrap();
        std::fs::write(generated.join("model/item.ts"), ORVAL_HEADER).unwrap();

        let error = regenerate(&generated, || anyhow::bail!("orval failed")).unwrap_err();

        assert_eq!(error.to_string(), "orval failed");
        assert_eq!(
            std::fs::read_to_string(generated.join("model/item.ts")).unwrap(),
            ORVAL_HEADER
        );
    }

    #[test]
    fn orval_resolves_from_the_package_or_a_hoisting_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("apps/web");
        std::fs::create_dir_all(&package).unwrap();
        assert_eq!(orval_bin(&package), None);
        std::fs::create_dir_all(dir.path().join("node_modules/.bin")).unwrap();
        std::fs::write(dir.path().join("node_modules/.bin/orval"), "").unwrap();
        assert_eq!(orval_bin(&package), Some(dir.path().join("node_modules/.bin/orval")));
    }

    #[test]
    fn the_query_client_invalidates_on_success_and_surfaces_every_failure() {
        let query = seam("src/lib/query.ts");
        assert!(query.contains("void queryClient.invalidateQueries();"));
        assert!(query.contains("mutation.meta?.silent !== true"));
        assert!(query.contains("mutation.meta?.expectedFailure !== true"));
        assert!(
            !regex::Regex::new(r"onError:[\s\S]*?meta\?\.silent\b")
                .unwrap()
                .is_match(&query)
        );
    }

    #[test]
    fn the_feedback_seam_falls_back_to_the_console() {
        let feedback = seam("src/lib/feedback.ts");
        assert!(feedback.contains("export function wireFeedback"));
        assert!(feedback.contains("export const feedback: FeedbackSink"));
        assert!(feedback.contains("console.error"));
    }
}
