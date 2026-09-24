//! `skies g feature` for Flutter: a ChangeNotifier ViewModel, a render-only View, and per-locale ARB copy.
//!
//! The ViewModel exposes one closed `AsyncState` and takes its data through an injected loader, so it never
//! imports the generated client. The row type is declared next to it, the Flutter twin of the React scaffold's
//! local entity interface, until the author swaps in the generated model. Tests are not scaffolded: the
//! feature's evidence is the E2E in its spec folder.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;
use serde_json::json;

use super::names::{pascal, snake};
use crate::web::names::singular;
use crate::web::scaffold::{render, write_new};

const VIEW_MODEL: &str = include_str!("../../templates/flutter/feature/view_model.dart");
const VIEW: &str = include_str!("../../templates/flutter/feature/view.dart");

/// Files grouped by where they land: `lib/features/<stem>/` and `lib/l10n/features/`.
pub struct FeatureFiles {
    pub stem: String,
    pub lib: Vec<(String, String)>,
    pub l10n: Vec<(String, String)>,
}

pub fn render_feature(name: &str) -> Result<FeatureFiles> {
    let stem = snake(name);
    if !stem.starts_with(|c: char| c.is_ascii_lowercase()) {
        bail!("'{name}' is not a usable feature name; start it with a letter");
    }
    let feature = pascal(name);
    let item = pascal(&singular(&stem));
    let ctx = context! { stem, feature, item };

    let lib = vec![
        (format!("{stem}_view_model.dart"), render(VIEW_MODEL, &ctx)?),
        (format!("{stem}_view.dart"), render(VIEW, &ctx)?),
    ];
    let copies = [
        (
            "pt_BR",
            "Nenhum item encontrado",
            "Não foi possível carregar. Tente novamente.",
        ),
        (
            "es",
            "No se encontraron elementos",
            "No se pudo cargar. Inténtalo de nuevo.",
        ),
        ("en", "No items found", "Could not load. Try again."),
    ];
    let mut l10n = Vec::new();
    for (locale, empty, error) in copies {
        let catalog = json!({
            format!("{stem}Title"): feature,
            format!("{stem}EmptyTitle"): empty,
            format!("{stem}LoadError"): error,
        });
        l10n.push((
            format!("{stem}_{locale}.arb"),
            format!("{}\n", serde_json::to_string_pretty(&catalog)?),
        ));
    }
    Ok(FeatureFiles { stem, lib, l10n })
}

pub fn scaffold(package: &Path, name: &str) -> Result<u8> {
    let files = render_feature(name)?;
    let lib = package.join("lib/features").join(&files.stem);
    let l10n = package.join("lib/l10n/features");
    let outputs: Vec<(PathBuf, String)> = files
        .lib
        .into_iter()
        .map(|(file, contents)| (lib.join(file), contents))
        .chain(
            files
                .l10n
                .into_iter()
                .map(|(file, contents)| (l10n.join(file), contents)),
        )
        .collect();
    write_new(&outputs)?;
    println!(
        "\nnext: wire the loader to the generated client in the composition root, then `skies i18n`."
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_mvvm_unit_and_its_copy_without_tests() {
        let files = render_feature("wallets").unwrap();

        let lib: Vec<&str> = files.lib.iter().map(|(name, _)| name.as_str()).collect();
        let l10n: Vec<&str> = files.l10n.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(lib, ["wallets_view_model.dart", "wallets_view.dart"]);
        assert_eq!(
            l10n,
            ["wallets_pt_BR.arb", "wallets_es.arb", "wallets_en.arb"]
        );

        let (model, view) = (&files.lib[0].1, &files.lib[1].1);
        assert!(model.contains("extends ChangeNotifier"));
        assert!(model.contains("AsyncState<List<Wallet>>"));
        assert!(model.contains("final class Wallet {"));
        assert!(view.contains("ResourceBuilder<List<Wallet>>"));
        assert!(!view.contains("package:dio") && !view.contains("Text("));
        for (_, contents) in files.lib.iter().chain(&files.l10n) {
            assert!(
                !contents.contains("@verify")
                    && !contents.contains("@e2e")
                    && !contents.contains("{{")
            );
        }
    }

    #[test]
    fn keeps_arb_keys_identical_across_locales() {
        let files = render_feature("UserWallets").unwrap();
        let keys: Vec<Vec<String>> = files
            .l10n
            .iter()
            .map(|(_, text)| {
                serde_json::from_str::<serde_json::Map<_, _>>(text)
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect()
            })
            .collect();
        assert_eq!(
            keys[0],
            [
                "user_walletsEmptyTitle",
                "user_walletsLoadError",
                "user_walletsTitle"
            ]
        );
        assert!(keys.iter().all(|k| k == &keys[0]));
    }

    #[test]
    fn writes_under_lib_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), "Profile").unwrap();
        assert!(
            dir.path()
                .join("lib/features/profile/profile_view_model.dart")
                .is_file()
        );
        assert!(
            dir.path()
                .join("lib/l10n/features/profile_en.arb")
                .is_file()
        );
        assert!(scaffold(dir.path(), "Profile").is_err());
    }
}
