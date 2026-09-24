//! The auth blueprints ship their proof as a spec folder, `.specs/<id>-<slug>/`.
//!
//! A spec is `spec.md` (the failure modes, `FM-1..n`) plus `e2e/` (black-box cases titled `FM-n: ...`). The
//! templates under `templates/dotnet/specs/<slug>/` name failure modes symbolically, `FM-[wrong-password]`, and
//! the numbers are assigned here, in the order `spec.md` lists them, after the flag regions are resolved. That
//! keeps the numbering dense in every variant: `--skip-tenancy` drops the tenancy modes without leaving holes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::{ApiProject, blueprint, embedded, first_csproj, text};

/// The Compile item that makes the tests project build every spec's E2E, relative to `tests/<App>.Tests`.
const SPEC_COMPILE: &str = "    <Compile Include=\"..\\..\\.specs\\*\\e2e\\**\\*.cs\" />";

/// Renders `templates/dotnet/specs/<slug>` into the next free `.specs/<id>-<slug>/` and makes sure the tests
/// project compiles it. Returns the new folder.
pub fn emit(project: &ApiProject, slug: &str, flags: blueprint::Flags) -> Result<PathBuf> {
    let specs_root = project.solution_root().join(".specs");
    let id = next_id(&specs_root)?;
    let folder = specs_root.join(format!("{id}-{slug}"));

    let files = render(slug, flags, &id, project.app_name(), &project.app_lower())?;
    for (relative, body) in &files {
        let path = folder.join(relative);
        text::write(&path, body)?;
        println!("created {}", path.display());
    }
    compile_specs_in_tests(project)?;
    note_missing_runner(project);
    Ok(folder)
}

/// The spec names `runner: api`; `skies proof record` needs that runner declared. New apps declare it; an older or
/// hand-edited manifest may not, so point at it rather than editing the manifest behind the owner's back.
fn note_missing_runner(project: &ApiProject) {
    let manifest = project.solution_root().join(crate::manifest::FILE_NAME);
    let declared =
        std::fs::read_to_string(&manifest).is_ok_and(|text| text.lines().any(|line| line.trim() == "[runners.api]"));
    if !declared {
        println!(
            "note: declare [runners.api] in {} so `skies proof record` can run this spec.",
            crate::manifest::FILE_NAME
        );
    }
}

/// Renders one spec template folder as (path relative to the spec folder, contents).
fn render(
    slug: &str,
    flags: blueprint::Flags,
    id: &str,
    app_name: &str,
    app_lower: &str,
) -> Result<Vec<(String, String)>> {
    let mut files: Vec<(String, String)> = embedded::dotnet_folder(&format!("specs/{slug}"))
        .into_iter()
        .filter(|(logical, _)| !skipped_by_flag(logical, flags))
        .map(|(logical, body)| {
            let rendered = blueprint::render(body, app_name, app_lower, flags).replace("__SPEC_ID__", id);
            (blueprint::render_path(&logical, app_name, app_lower), rendered)
        })
        .collect();
    number_failure_modes(&mut files)?;
    Ok(files)
}

/// The next spec id: one past the highest numeric prefix under `.specs/`, four digits, starting at 0001.
pub fn next_id(specs_root: &Path) -> Result<String> {
    let mut highest = 0u32;
    if specs_root.is_dir() {
        for entry in std::fs::read_dir(specs_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(number) = digits.parse::<u32>() {
                highest = highest.max(number);
            }
        }
    }
    Ok(format!("{:04}", highest + 1))
}

/// Spec files that exist only in one variant are named for their region: `Tenancy.cs` and `CookieDelivery.cs`.
fn skipped_by_flag(logical: &str, flags: blueprint::Flags) -> bool {
    (!flags.tenancy && logical.ends_with("e2e/Tenancy.cs.cstmpl"))
        || (!flags.cookies && logical.ends_with("e2e/CookieDelivery.cs.cstmpl"))
}

/// Replaces every `FM-[key]` with `FM-n`, numbering keys in the order `spec.md` first mentions them. A key an
/// E2E case uses but the spec never lists is a template bug, so it fails loudly instead of emitting a case the
/// proof engine could not map.
fn number_failure_modes(files: &mut [(String, String)]) -> Result<()> {
    let Some((_, spec)) = files.iter().find(|(path, _)| path == "spec.md") else {
        bail!("spec template has no spec.md");
    };
    let mut numbers: HashMap<String, usize> = HashMap::new();
    for key in failure_mode_keys(spec) {
        let next = numbers.len() + 1;
        numbers.entry(key).or_insert(next);
    }
    for (path, body) in files.iter_mut() {
        for key in failure_mode_keys(body) {
            let Some(number) = numbers.get(&key) else {
                bail!("{path} names FM-[{key}], which spec.md does not list");
            };
            *body = body.replace(&format!("FM-[{key}]"), &format!("FM-{number}"));
        }
    }
    Ok(())
}

/// The symbolic failure-mode keys in `text`, in order of appearance.
pub fn failure_mode_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("FM-[") {
        let after = &rest[start + 4..];
        let Some(end) = after.find(']') else { break };
        keys.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    keys
}

/// Adds the `.specs` Compile item to the tests project when an older app lacks it, so the emitted cases build.
fn compile_specs_in_tests(project: &ApiProject) -> Result<()> {
    let Some(csproj) = first_csproj(&project.test_dir()) else {
        println!(
            "note: no test project at {} — compile the spec E2E with {}",
            project.test_dir().display(),
            SPEC_COMPILE.trim()
        );
        return Ok(());
    };
    let current = text::read(&csproj)?;
    if current.contains(".specs\\") || current.contains(".specs/") {
        return Ok(());
    }
    let nl = text::newline_of(&current);
    let block = format!("  <ItemGroup>{nl}{SPEC_COMPILE}{nl}  </ItemGroup>{nl}{nl}");
    std::fs::write(&csproj, text::insert_before_project_end(&current, &block))?;
    println!(
        "added the .specs E2E folders to {}",
        csproj.file_name().unwrap_or_default().to_string_lossy()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_continue_after_the_highest_numbered_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_id(&dir.path().join(".specs")).unwrap(), "0001");
        for name in ["0003-auth", "0010-orders", "notes", "0002-x"] {
            std::fs::create_dir_all(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("0042-not-a-folder.md"), "").unwrap();
        assert_eq!(next_id(dir.path()).unwrap(), "0011");
    }

    #[test]
    fn failure_modes_are_numbered_by_their_order_in_the_spec() {
        let mut files = vec![
            (
                "spec.md".to_string(),
                "- FM-[b] second\n- FM-[a] third\n- FM-[c] first?\n".to_string(),
            ),
            (
                "e2e/X.cs".to_string(),
                "\"FM-[a]: x\" \"FM-[c]: y\" \"FM-[b]: z\"".to_string(),
            ),
        ];
        number_failure_modes(&mut files).unwrap();
        assert_eq!(files[0].1, "- FM-1 second\n- FM-2 third\n- FM-3 first?\n");
        assert_eq!(files[1].1, "\"FM-2: x\" \"FM-3: y\" \"FM-1: z\"");
    }

    /// The proof engine's one consistency rule, checked on every shipped spec in every variant: each listed
    /// failure mode has at least one case, each case maps to a listed mode, and numbering has no holes.
    #[test]
    fn every_shipped_spec_maps_cases_to_failure_modes_one_to_one() {
        let variants = [
            blueprint::Flags::DEFAULT,
            blueprint::Flags {
                tenancy: false,
                cookies: true,
            },
            blueprint::Flags {
                tenancy: true,
                cookies: false,
            },
            blueprint::Flags {
                tenancy: false,
                cookies: false,
            },
        ];
        for slug in ["auth", "auth-otp", "auth-oauth", "auth-email"] {
            for flags in variants {
                let files = render(slug, flags, "0007", "Acme", "acme").unwrap();
                let spec = &files.iter().find(|(path, _)| path == "spec.md").unwrap().1;
                assert!(
                    spec.starts_with("---\nid: \"0007\"\nrunner: api\n---\n"),
                    "{slug} frontmatter"
                );
                let listed: Vec<usize> = spec
                    .lines()
                    .filter_map(|line| line.strip_prefix("- FM-"))
                    .map(|rest| rest.split(' ').next().unwrap().parse().unwrap())
                    .collect();
                assert_eq!(
                    listed,
                    (1..=listed.len()).collect::<Vec<_>>(),
                    "{slug} {flags:?}: dense numbering"
                );

                let mut covered = std::collections::BTreeSet::new();
                for (path, body) in files.iter().filter(|(path, _)| path.starts_with("e2e/")) {
                    assert!(body.contains("namespace Specs.S0007;"), "{path}");
                    assert!(!body.contains("FM-["), "{path} left a symbolic failure mode");
                    for case in body.split("DisplayName = \"FM-").skip(1) {
                        let number: usize = case.split(':').next().unwrap().parse().unwrap();
                        assert!(listed.contains(&number), "{slug} {path}: FM-{number} is not in spec.md");
                        covered.insert(number);
                    }
                }
                assert_eq!(
                    covered.into_iter().collect::<Vec<_>>(),
                    listed,
                    "{slug} {flags:?}: every FM has a case"
                );
            }
        }
    }

    #[test]
    fn a_case_for_an_unlisted_failure_mode_is_rejected() {
        let mut files = vec![
            ("spec.md".to_string(), "- FM-[a] listed\n".to_string()),
            ("e2e/X.cs".to_string(), "\"FM-[ghost]: x\"".to_string()),
        ];
        assert!(
            number_failure_modes(&mut files)
                .unwrap_err()
                .to_string()
                .contains("ghost")
        );
    }
}
