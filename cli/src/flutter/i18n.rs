//! Flutter i18n: per-feature ARB catalogs, their key parity (SKYFL011), and the assembled gen_l10n inputs.
//!
//! Features keep their copy next to each other in `lib/l10n/features/<family>_<locale>.arb`. `gen_l10n` wants one
//! file per locale, so `skies i18n` merges each locale's catalogs into `lib/l10n/app_<locale>.arb`, refusing when a
//! family is missing keys in some locale (a silent untranslated string) or two families claim the same key.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value};

/// Where feature catalogs live and where the assembled catalogs go, relative to the package.
pub const FEATURES_DIR: &str = "lib/l10n/features";
pub const OUTPUT_DIR: &str = "lib/l10n";

/// The locale suffix of a catalog's file name: a 2–3 letter language, optionally a `_Region`/`_Script` part
/// (`_en`, `_pt_BR`, `_zh_Hant`). Any language the app writes is one; none is privileged.
fn locale_pattern() -> Regex {
    Regex::new(r"_([a-z]{2,3}(?:_[A-Z][A-Za-z0-9]+)?)\.arb$").expect("static pattern")
}

/// The locale a new package starts with when it declares none.
pub const DEFAULT_LOCALE: &str = "en";

/// The app's locale set, read from the feature catalogs it already has (`lib/l10n/features/*_<locale>.arb`), sorted;
/// a package with none yet gets the single [`DEFAULT_LOCALE`]. A scaffold writes one catalog per locale the app
/// already speaks, so adding a feature never adds or drops a language.
pub fn locales(package: &Path) -> Vec<String> {
    let pattern = locale_pattern();
    let found: BTreeSet<String> = find_arb(&package.join(FEATURES_DIR))
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            pattern.captures(&name).map(|captures| captures[1].to_string())
        })
        .collect();
    if found.is_empty() {
        vec![DEFAULT_LOCALE.to_string()]
    } else {
        found.into_iter().collect()
    }
}

/// One parsed ARB file.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub path: PathBuf,
    pub value: Map<String, Value>,
}

/// A family whose locales disagree on keys: which file lacks which keys.
#[derive(Debug, PartialEq)]
pub struct ParityGap {
    pub path: PathBuf,
    pub missing: Vec<String>,
}

/// Every `.arb` under `root`, sorted, skipping build output.
pub fn find_arb(root: &Path) -> Vec<PathBuf> {
    super::files_with_extension(root, "arb")
}

pub fn read_catalog(path: &Path) -> Result<Catalog> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Catalog {
        path: path.to_path_buf(),
        value,
    })
}

/// Compares the message keys (not `@` metadata) of every locale within each family.
pub fn check_parity(catalogs: &[Catalog]) -> Vec<ParityGap> {
    let locale = locale_pattern();
    let mut families: BTreeMap<String, Vec<(&Catalog, BTreeSet<&str>)>> = BTreeMap::new();
    for catalog in catalogs {
        let name = catalog.path.file_name().unwrap_or_default().to_string_lossy();
        if !locale.is_match(&name) {
            continue;
        }
        let family = locale.replace(&name, "").into_owned();
        let keys = catalog
            .value
            .keys()
            .map(String::as_str)
            .filter(|key| !key.starts_with('@'))
            .collect();
        families.entry(family).or_default().push((catalog, keys));
    }
    let mut gaps = Vec::new();
    for entries in families.values() {
        let union: BTreeSet<&str> = entries.iter().flat_map(|(_, keys)| keys.iter().copied()).collect();
        for (catalog, keys) in entries {
            let missing: Vec<String> = union.difference(keys).map(|key| key.to_string()).collect();
            if !missing.is_empty() {
                gaps.push(ParityGap {
                    path: catalog.path.clone(),
                    missing,
                });
            }
        }
    }
    gaps
}

/// Merges catalogs into one map per locale. A key claimed by two catalogs of the same locale is an error because
/// gen_l10n would silently keep only one of them.
pub fn assemble_arb(catalogs: &[Catalog]) -> Result<BTreeMap<String, Map<String, Value>>> {
    let pattern = locale_pattern();
    let mut output: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for catalog in catalogs {
        let name = catalog.path.file_name().unwrap_or_default().to_string_lossy();
        let Some(captures) = pattern.captures(&name) else {
            continue;
        };
        let locale = captures[1].to_string();
        let merged = output.entry(locale.clone()).or_default();
        for (key, value) in &catalog.value {
            if merged.contains_key(key) {
                bail!("duplicate ARB key {key} in locale {locale}");
            }
            merged.insert(key.clone(), value.clone());
        }
    }
    // gen_l10n refuses a regional locale (`pt_BR`) without its base (`pt`) to fall back to. When features only
    // write the regional catalog, the base is the same copy.
    let regional: Vec<(String, Map<String, Value>)> = output
        .iter()
        .filter_map(|(locale, map)| locale.split_once('_').map(|(base, _)| (base.to_string(), map.clone())))
        .collect();
    for (base, map) in regional {
        output.entry(base).or_insert(map);
    }
    Ok(output)
}

/// `skies i18n` for a Flutter package.
pub fn assemble(package: &Path) -> Result<u8> {
    let root = package.join(FEATURES_DIR);
    let catalogs = find_arb(&root)
        .iter()
        .map(|path| read_catalog(path))
        .collect::<Result<Vec<_>>>()?;
    if catalogs.is_empty() {
        bail!("no *.arb catalogs found under {}", root.display());
    }
    let gaps = check_parity(&catalogs);
    if !gaps.is_empty() {
        for gap in &gaps {
            eprintln!(
                "SKYFL011 {}: missing keys {}",
                gap.path.display(),
                gap.missing.join(", ")
            );
        }
        return Ok(1);
    }
    let output = package.join(OUTPUT_DIR);
    std::fs::create_dir_all(&output)?;
    for (locale, value) in assemble_arb(&catalogs)? {
        let path = output.join(format!("app_{locale}.arb"));
        std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&value)?))?;
        println!("assembled {}", path.display());
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(path: &str, value: Value) -> Catalog {
        Catalog {
            path: PathBuf::from(path),
            value: value.as_object().unwrap().clone(),
        }
    }

    fn wallets() -> Vec<Catalog> {
        vec![
            catalog(
                "wallets_pt_BR.arb",
                json!({ "title": "Carteiras", "apiError_missing": "Ausente" }),
            ),
            catalog(
                "wallets_en.arb",
                json!({ "title": "Wallets", "apiError_missing": "Missing" }),
            ),
        ]
    }

    #[test]
    fn parity_and_assembly_preserve_every_feature_key() {
        let catalogs = wallets();
        assert!(check_parity(&catalogs).is_empty());
        assert_eq!(assemble_arb(&catalogs).unwrap()["en"], catalogs[1].value);
    }

    #[test]
    fn parity_names_the_file_missing_a_key_and_ignores_metadata() {
        let catalogs = vec![
            catalog("x_en.arb", json!({ "title": "Title", "empty": "Empty", "@title": {} })),
            catalog("x_pt_BR.arb", json!({ "title": "Título" })),
        ];
        assert_eq!(
            check_parity(&catalogs),
            [ParityGap {
                path: PathBuf::from("x_pt_BR.arb"),
                missing: vec!["empty".into()]
            }]
        );
    }

    #[test]
    fn assembly_rejects_a_key_two_features_claim() {
        let catalogs = vec![
            catalog("a_en.arb", json!({ "title": "A" })),
            catalog("b_en.arb", json!({ "title": "B" })),
        ];
        assert!(
            assemble_arb(&catalogs)
                .unwrap_err()
                .to_string()
                .contains("duplicate ARB key title")
        );
    }

    #[test]
    fn the_locale_set_is_the_apps_own_and_defaults_to_english() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(locales(dir.path()), ["en"]);

        let features = dir.path().join(FEATURES_DIR);
        std::fs::create_dir_all(&features).unwrap();
        for file in [
            "common_fr.arb",
            "common_de_CH.arb",
            "user_wallets_fr.arb",
            "user_wallets_de_CH.arb",
        ] {
            std::fs::write(features.join(file), "{}").unwrap();
        }
        assert_eq!(locales(dir.path()), ["de_CH", "fr"]);
    }

    #[test]
    fn parity_reads_any_locale_suffix() {
        let catalogs = vec![
            catalog("x_fr.arb", json!({ "title": "Titre", "empty": "Vide" })),
            catalog("x_de_CH.arb", json!({ "title": "Titel" })),
        ];
        assert_eq!(check_parity(&catalogs)[0].path, PathBuf::from("x_de_CH.arb"));
    }

    #[test]
    fn assembles_feature_catalogs_into_gen_l10n_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let features = dir.path().join(FEATURES_DIR);
        std::fs::create_dir_all(&features).unwrap();
        std::fs::write(features.join("wallets_en.arb"), r#"{"walletsTitle":"Wallets"}"#).unwrap();
        std::fs::write(features.join("wallets_pt_BR.arb"), r#"{"walletsTitle":"Carteiras"}"#).unwrap();

        assert_eq!(assemble(dir.path()).unwrap(), 0);

        let en = std::fs::read_to_string(dir.path().join("lib/l10n/app_en.arb")).unwrap();
        assert_eq!(en, "{\n  \"walletsTitle\": \"Wallets\"\n}\n");
        assert!(dir.path().join("lib/l10n/app_pt_BR.arb").is_file());
        let base = std::fs::read_to_string(dir.path().join("lib/l10n/app_pt.arb")).unwrap();
        assert!(
            base.contains("Carteiras"),
            "the regional copy doubles as the gen_l10n fallback"
        );
    }
}
