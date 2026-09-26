//! `skies i18n` for React: assembles every feature's `*.i18n.ts` catalog into one generated resource module.
//!
//! The output imports each catalog and composes `resources` (locale → namespace), the tree an app otherwise wires
//! by hand. It typechecks, so a renamed or removed catalog fails the build; key parity inside a catalog stays with
//! SKYFE011 in the ESLint plugin.
//!
//! The locale set is read from the catalogs, never assumed: each `export const <locale> =` is one locale, keyed in
//! `resources` by its language (`enUS`, `en_US`, and `en` all become `en`), and every catalog must declare the same
//! set, since a namespace missing a language would silently fall back at runtime.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;

use super::names::ident;
use super::scaffold::relative_path;

/// The generated module, relative to the package's source root.
pub const OUTPUT: &str = "i18n/resources.generated.ts";

/// A top-level `export const <ident> =`: one locale of a catalog.
static LOCALE_EXPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^export\s+const\s+(?<locale>[A-Za-z][A-Za-z0-9_]*)\s*(?::[^=]*)?=").expect("locale regex")
});

/// One discovered catalog: its namespace and the extensionless import path from the output module.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalog {
    pub namespace: String,
    pub import_path: String,
}

/// The locale exports a catalog declares, in declaration order.
pub fn catalog_locales(source: &str) -> Vec<String> {
    LOCALE_EXPORT
        .captures_iter(source)
        .map(|capture| capture["locale"].to_string())
        .collect()
}

/// The i18next language key of a locale export: its leading lowercase letters (`enUS` → `en`, `pt_BR` → `pt`).
pub fn language_key(locale: &str) -> String {
    let lower: String = locale.chars().take_while(|c| c.is_ascii_lowercase()).collect();
    if lower.is_empty() {
        locale.to_ascii_lowercase()
    } else {
        lower
    }
}

/// The one locale set every catalog shares, or an error naming the catalog that differs. Order is the first
/// catalog's (sorted by path), so the output is stable.
fn shared_locales(paths: &[PathBuf]) -> Result<Vec<String>> {
    let mut expected: Option<(Vec<String>, &PathBuf)> = None;
    for path in paths {
        let source = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let locales = catalog_locales(&source);
        if locales.is_empty() {
            bail!("{} exports no locale (`export const en = {{ ... }}`)", path.display());
        }
        match &expected {
            None => {
                let mut keys: Vec<String> = locales.iter().map(|l| language_key(l)).collect();
                keys.sort();
                if keys.windows(2).any(|pair| pair[0] == pair[1]) {
                    bail!(
                        "{} declares two locales of one language ({}); resources are keyed by language",
                        path.display(),
                        locales.join(", ")
                    );
                }
                expected = Some((locales, path));
            }
            Some((first, first_path)) => {
                let mut ours = locales.clone();
                let mut theirs = first.clone();
                ours.sort();
                theirs.sort();
                if ours != theirs {
                    bail!(
                        "{} declares the locales {} but {} declares {}; every catalog needs the same set",
                        path.display(),
                        locales.join(", "),
                        first_path.display(),
                        first.join(", ")
                    );
                }
            }
        }
    }
    Ok(expected.map(|(locales, _)| locales).unwrap_or_default())
}

pub fn assemble(package: &Path) -> Result<u8> {
    let src = package.join("src");
    let root = if src.is_dir() { src } else { package.to_path_buf() };
    let output = root.join(OUTPUT);
    let out_dir = output.parent().context("output has a parent")?.to_path_buf();

    let mut paths = Vec::new();
    find_catalogs(&root, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        bail!("no *.i18n.ts catalogs found under {}", root.display());
    }
    let locales = shared_locales(&paths)?;
    let catalogs: Vec<Catalog> = paths
        .iter()
        .map(|path| {
            let file = path.file_name().unwrap_or_default().to_string_lossy();
            let namespace = file.trim_end_matches(".i18n.ts").to_string();
            let import_path = relative_path(&out_dir, &path.with_file_name(format!("{namespace}.i18n")));
            Catalog { namespace, import_path }
        })
        .collect();

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(&output, render_resources(&catalogs, &locales))
        .with_context(|| format!("writing {}", output.display()))?;
    println!("assembled {} catalog(s) -> {}", catalogs.len(), output.display());
    for catalog in &catalogs {
        println!("  {}  ({})", catalog.namespace, catalog.import_path);
    }
    Ok(0)
}

/// Every `*.i18n.ts` under `dir`, skipping `node_modules`.
pub fn find_catalogs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if entry.file_type()?.is_dir() {
            if name != "node_modules" {
                find_catalogs(&path, out)?;
            }
        } else if name.to_string_lossy().ends_with(".i18n.ts") {
            out.push(path);
        }
    }
    Ok(())
}

/// Renders the resource module over the catalogs' shared `locales`. Catalogs are sorted by namespace so the file is
/// stable across runs and machines.
pub fn render_resources(catalogs: &[Catalog], locales: &[String]) -> String {
    let mut sorted = catalogs.to_vec();
    sorted.sort_by(|a, b| a.namespace.cmp(&b.namespace));

    let mut out = String::from(
        "// GENERATED by `skies i18n` — do not edit. Re-run to regenerate after adding/removing a feature.\n",
    );
    for catalog in &sorted {
        let id = ident(&catalog.namespace);
        let names: Vec<String> = locales
            .iter()
            .map(|locale| format!("{locale} as {id}_{locale}"))
            .collect();
        out += &format!("import {{ {} }} from \"{}\";\n", names.join(", "), catalog.import_path);
    }
    out += "\nexport const resources = {\n";
    for locale in locales {
        out += &format!("  {}: {{\n", language_key(locale));
        for catalog in &sorted {
            let key = serde_json::to_string(&catalog.namespace).unwrap_or_default();
            out += &format!("    {key}: {}_{locale},\n", ident(&catalog.namespace));
        }
        out += "  },\n";
    }
    out += "} as const;\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(namespace: &str, import_path: &str) -> Catalog {
        Catalog {
            namespace: namespace.into(),
            import_path: import_path.into(),
        }
    }

    fn set(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn composes_the_locale_namespace_tree_in_a_stable_order() {
        let out = render_resources(
            &[
                catalog("items", "../features/items/items.i18n"),
                catalog("bookings", "../features/bookings/bookings.i18n"),
            ],
            &set(&["ptBR", "esES", "enUS"]),
        );

        assert!(out.contains(
            "import { ptBR as items_ptBR, esES as items_esES, enUS as items_enUS } from \"../features/items/items.i18n\";"
        ));
        assert!(out.contains("    \"bookings\": bookings_enUS,\n"));
        assert!(out.contains("export const resources"));
        assert!(out.find("bookings_ptBR").unwrap() < out.find("items_ptBR").unwrap());
        assert!(out.contains("  pt: {\n    \"bookings\": bookings_ptBR,"));
    }

    #[test]
    fn reads_locale_exports_and_keys_them_by_language() {
        let source = "export const en = {} as const;\nexport const pt_BR: Catalog = {};\nconst local = 1;\n";
        assert_eq!(catalog_locales(source), ["en", "pt_BR"]);
        assert_eq!(language_key("enUS"), "en");
        assert_eq!(language_key("en_US"), "en");
        assert_eq!(language_key("en"), "en");
        assert_eq!(language_key("pt_BR"), "pt");
    }

    #[test]
    fn a_single_locale_app_gets_a_single_language() {
        let out = render_resources(&[catalog("items", "./items.i18n")], &set(&["en"]));
        assert!(out.contains("import { en as items_en } from \"./items.i18n\";"));
        assert!(out.contains("  en: {\n    \"items\": items_en,\n  },\n} as const;"));
        assert!(!out.contains("pt:") && !out.contains("enUS"));
    }

    #[test]
    fn refuses_catalogs_whose_locales_differ() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/a")).unwrap();
        std::fs::create_dir_all(dir.path().join("src/b")).unwrap();
        std::fs::write(
            dir.path().join("src/a/a.i18n.ts"),
            "export const en = {};\nexport const pt = {};\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("src/b/b.i18n.ts"), "export const en = {};\n").unwrap();

        let error = assemble(dir.path()).unwrap_err().to_string();

        assert!(error.contains("b.i18n.ts declares the locales en"), "{error}");
        assert!(!dir.path().join("src").join(OUTPUT).exists());
    }

    #[test]
    fn makes_a_js_safe_identifier_from_a_kebab_namespace() {
        let out = render_resources(&[catalog("user-profile", "./up.i18n")], &set(&["enUS"]));
        assert!(out.contains("user_profile_enUS"));
        assert!(out.contains("\"user-profile\": user_profile_enUS,"));
    }

    #[test]
    fn discovers_catalogs_under_src_and_writes_the_module() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/items")).unwrap();
        std::fs::write(dir.path().join("src/items/items.i18n.ts"), "export const enUS = {};").unwrap();

        assemble(dir.path()).unwrap();

        let out = std::fs::read_to_string(dir.path().join("src").join(OUTPUT)).unwrap();
        assert!(out.contains("import { enUS as items_enUS } from \"../items/items.i18n\";"));
        assert!(out.contains("  en: {\n"));
    }

    #[test]
    fn fails_when_there_is_nothing_to_assemble() {
        let dir = tempfile::tempdir().unwrap();
        assert!(assemble(dir.path()).is_err());
    }
}
