//! `skies g client` for a React web package: the hand-owned client seams plus an orval run.
//!
//! orval generates the hooks. What it cannot generate are the files every hook calls through: the mutator (auth
//! injection, the injectable base URL, the `X-Client: web` header that turns on the cookie session), the
//! QueryClient with the write-side defaults, the feedback seam, and the orval config itself. Those are written
//! once and then owned by the app; orval's output is regenerated on every run.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;

use super::contract;
use super::scaffold::{relative_path, render, run, tree_hash, write_missing};

const MUTATOR: &str = include_str!("../../templates/react/client/skies-client.ts");
const FEEDBACK: &str = include_str!("../../templates/react/client/feedback.ts");
const QUERY: &str = include_str!("../../templates/react/client/query.ts");
const ORVAL_CONFIG: &str = include_str!("../../templates/react/client/orval.config.ts");

/// The hand-owned files, relative to the package root.
pub fn render_seams(name: &str, contract: &str) -> Result<Vec<(&'static str, String)>> {
    Ok(vec![
        ("src/lib/skies-client.ts", MUTATOR.to_string()),
        ("src/lib/feedback.ts", FEEDBACK.to_string()),
        ("src/lib/query.ts", QUERY.to_string()),
        ("orval.config.ts", render(ORVAL_CONFIG, context! { name, contract })?),
    ])
}

pub fn generate(package: &Path) -> Result<u8> {
    let contract = contract::for_package(package)?;
    let contract_path = relative_path(package, &contract.path);
    let seams: Vec<(PathBuf, String)> = render_seams(&contract.name, &contract_path)?
        .into_iter()
        .map(|(file, contents)| (package.join(file), contents))
        .collect();
    write_missing(&seams)?;

    if !package
        .join("node_modules/.bin")
        .join(if cfg!(windows) { "orval.cmd" } else { "orval" })
        .exists()
    {
        bail!(
            "orval is not installed in {}; run `npm install --save-dev orval` there",
            package.display()
        );
    }
    let generated = package.join("src/client.gen");
    let before = tree_hash(&generated);
    run(
        npx(),
        &["--no-install", "orval", "--config", "orval.config.ts"],
        package,
    )?;
    let after = tree_hash(&generated);
    let verdict = if before == after { "unchanged" } else { "updated" };
    println!("client.gen {verdict} from {}", contract.path.display());
    Ok(0)
}

fn npx() -> &'static str {
    if cfg!(windows) { "npx.cmd" } else { "npx" }
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
