//! `skies g feature` for Flutter: a ChangeNotifier ViewModel, a render-only View, and per-locale ARB copy.
//!
//! Two kinds, like the React scaffold. `list` exposes one closed `AsyncState` and takes its data through an
//! injected loader; the row type is declared next to it until the author swaps in the generated model. `form` owns
//! the field values and their validation, submits through `submitOrReveal` into an injected command port, and
//! exposes the command as an `AsyncState` (pending, sent, failed). Neither imports the generated client: the
//! composition root wires the port to it (inside the app's `MutationBoundary` for a command). Tests are not
//! scaffolded: the feature's evidence is the E2E in its spec folder.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;
use serde_json::json;

use super::i18n;
use super::names::{pascal, snake};
use crate::web::FeatureKind;
use crate::web::names::singular;
use crate::web::scaffold::{render, write_new};

const LIST_VIEW_MODEL: &str = include_str!("../../templates/flutter/feature/list/view_model.dart");
const LIST_VIEW: &str = include_str!("../../templates/flutter/feature/list/view.dart");
const FORM_VIEW_MODEL: &str = include_str!("../../templates/flutter/feature/form/view_model.dart");
const FORM_VIEW: &str = include_str!("../../templates/flutter/feature/form/view.dart");

/// Files grouped by where they land: `lib/features/<stem>/` and `lib/l10n/features/`.
pub struct FeatureFiles {
    pub stem: String,
    pub lib: Vec<(String, String)>,
    pub l10n: Vec<(String, String)>,
}

/// The copy each kind starts with, per locale: `(key suffix, text)` pairs; every locale carries the same keys. The
/// set of locales is the app's (see [`i18n::locales`]); a language with no starter copy here gets the English text
/// to translate, so parity holds from the first build.
fn copy(kind: FeatureKind, locale: &str) -> Vec<(&'static str, &'static str)> {
    let language = locale.split('_').next().unwrap_or(locale);
    match (kind, language) {
        (FeatureKind::List, "pt") => vec![
            ("EmptyTitle", "Nenhum item encontrado"),
            ("LoadError", "Não foi possível carregar. Tente novamente."),
        ],
        (FeatureKind::List, "es") => vec![
            ("EmptyTitle", "No se encontraron elementos"),
            ("LoadError", "No se pudo cargar. Inténtalo de nuevo."),
        ],
        (FeatureKind::List, _) => vec![
            ("EmptyTitle", "No items found"),
            ("LoadError", "Could not load. Try again."),
        ],
        (FeatureKind::Form, "pt") => vec![
            ("IdLabel", "Id"),
            ("IdInvalid", "Informe um id válido."),
            ("SubmitError", "Não foi possível concluir. Tente novamente."),
            ("DoneTitle", "Concluído"),
        ],
        (FeatureKind::Form, "es") => vec![
            ("IdLabel", "Id"),
            ("IdInvalid", "Introduce un id válido."),
            ("SubmitError", "No pudimos completarlo. Inténtalo de nuevo."),
            ("DoneTitle", "Completado"),
        ],
        (FeatureKind::Form, _) => vec![
            ("IdLabel", "Id"),
            ("IdInvalid", "Enter a valid id."),
            ("SubmitError", "We couldn't complete it. Try again."),
            ("DoneTitle", "Done"),
        ],
    }
}

/// The default (`list`) unit in the default locale, the shape the SKYFL rule contract checks.
#[cfg(test)]
pub fn render_feature(name: &str) -> Result<FeatureFiles> {
    render_feature_of(name, FeatureKind::List, &[i18n::DEFAULT_LOCALE.to_string()])
}

/// Renders the unit with one ARB catalog per locale in `locales`.
pub fn render_feature_of(name: &str, kind: FeatureKind, locales: &[String]) -> Result<FeatureFiles> {
    let stem = snake(name);
    if !stem.starts_with(|c: char| c.is_ascii_lowercase()) {
        bail!("'{name}' is not a usable feature name; start it with a letter");
    }
    let feature = pascal(name);
    let item = pascal(&singular(&stem));
    let ctx = context! { stem, feature, item };
    let (view_model, view) = match kind {
        FeatureKind::List => (LIST_VIEW_MODEL, LIST_VIEW),
        FeatureKind::Form => (FORM_VIEW_MODEL, FORM_VIEW),
    };

    let lib = vec![
        (format!("{stem}_view_model.dart"), render(view_model, &ctx)?),
        (format!("{stem}_view.dart"), render(view, &ctx)?),
    ];
    let mut l10n = Vec::new();
    for locale in locales {
        let mut catalog = serde_json::Map::new();
        catalog.insert(format!("{stem}Title"), json!(feature));
        if kind == FeatureKind::Form {
            catalog.insert(format!("{stem}Submit"), json!(feature));
        }
        for (key, text) in copy(kind, locale) {
            catalog.insert(format!("{stem}{key}"), json!(text));
        }
        l10n.push((
            format!("{stem}_{locale}.arb"),
            format!("{}\n", serde_json::to_string_pretty(&catalog)?),
        ));
    }
    Ok(FeatureFiles { stem, lib, l10n })
}

pub fn scaffold(package: &Path, name: &str, kind: FeatureKind) -> Result<u8> {
    let files = render_feature_of(name, kind, &i18n::locales(package))?;
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
    let port = match kind {
        FeatureKind::List => "the loader to the generated client's List operation",
        FeatureKind::Form => "the send port to the generated client's operation, inside the app's MutationBoundary,",
    };
    println!("\nnext: wire {port} in the composition root, then `skies i18n`.");
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
            ["wallets_en.arb"],
            "a package with no catalogs speaks English only"
        );

        let (model, view) = (&files.lib[0].1, &files.lib[1].1);
        assert!(model.contains("extends ChangeNotifier"));
        assert!(model.contains("AsyncState<List<Wallet>>"));
        assert!(model.contains("final class Wallet {"));
        assert!(view.contains("ResourceBuilder<List<Wallet>>"));
        assert!(!view.contains("package:dio") && !view.contains("Text("));
        for (_, contents) in files.lib.iter().chain(&files.l10n) {
            assert!(!contents.contains("@verify") && !contents.contains("@e2e") && !contents.contains("{{"));
        }
    }

    #[test]
    fn keeps_arb_keys_identical_across_locales() {
        let locales = ["en", "es", "pt_BR"].map(String::from);
        let files = render_feature_of("UserWallets", FeatureKind::List, &locales).unwrap();
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
            ["user_walletsEmptyTitle", "user_walletsLoadError", "user_walletsTitle"]
        );
        assert!(keys.iter().all(|k| k == &keys[0]));
    }

    #[test]
    fn a_feature_follows_the_locales_the_app_already_speaks() {
        let dir = tempfile::tempdir().unwrap();
        let l10n = dir.path().join("lib/l10n/features");
        std::fs::create_dir_all(&l10n).unwrap();
        std::fs::write(l10n.join("common_fr.arb"), r#"{"appTitle":"App"}"#).unwrap();
        std::fs::write(l10n.join("common_pt_BR.arb"), r#"{"appTitle":"App"}"#).unwrap();

        scaffold(dir.path(), "Profile", FeatureKind::List).unwrap();

        let mut written: Vec<String> = std::fs::read_dir(&l10n)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("profile_"))
            .collect();
        written.sort();
        assert_eq!(written, ["profile_fr.arb", "profile_pt_BR.arb"]);
        let portuguese = std::fs::read_to_string(l10n.join("profile_pt_BR.arb")).unwrap();
        assert!(portuguese.contains("Nenhum item encontrado"));
    }

    #[test]
    fn scaffolds_cite_no_rule() {
        let rule = regex::Regex::new(r"SKY(FE|FL|WS)?[0-9]{3,4}").unwrap();
        for kind in [FeatureKind::List, FeatureKind::Form] {
            let files = render_feature_of("wallets", kind, &["en".to_string()]).unwrap();
            for (name, contents) in files.lib.iter().chain(&files.l10n) {
                assert!(!rule.is_match(contents), "{name} cites a rule id");
            }
        }
        for template in [
            include_str!("../../templates/flutter/client/mutations.dart"),
            include_str!("../../templates/flutter/client/session.dart"),
            include_str!("../../templates/flutter/client/skies_client.dart"),
        ] {
            assert!(!rule.is_match(template));
        }
    }

    #[test]
    fn writes_under_lib_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), "Profile", FeatureKind::List).unwrap();
        assert!(
            dir.path()
                .join("lib/features/profile/profile_view_model.dart")
                .is_file()
        );
        assert!(dir.path().join("lib/l10n/features/profile_en.arb").is_file());
        assert!(scaffold(dir.path(), "Profile", FeatureKind::Form).is_err());
    }

    #[test]
    fn a_form_owns_its_fields_and_the_command_state() {
        let locales = ["de", "en"].map(String::from);
        let files = render_feature_of("wallet-transfer", FeatureKind::Form, &locales).unwrap();
        let (model, view) = (&files.lib[0].1, &files.lib[1].1);

        assert!(model.contains("enum WalletTransferField { id }"));
        assert!(model.contains("typedef SendWalletTransfer = Future<void> Function({required String id});"));
        assert!(model.contains("AsyncState<bool> _submission = const AsyncEmpty<bool>();"));
        assert!(model.contains("await submitOrReveal<WalletTransferField>("));
        assert!(model.contains("_submission = AsyncFailure<bool>(error, stackTrace, retry: submit);"));
        assert!(!model.contains("package:flutter/widgets.dart") && !model.contains("BuildContext"));
        assert!(view.contains("import 'wallet_transfer_view_model.dart';"));
        assert!(view.contains("ResourceBuilder<bool>(") && view.contains("ready: (context, _) => done(context),"));
        assert!(!view.contains("Text(") && !view.contains("package:dio"));

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
        assert!(keys.iter().all(|k| k == &keys[0]));
        assert!(keys[0].contains(&"wallet_transferSubmitError".to_string()));
        for (_, contents) in files.lib.iter().chain(&files.l10n) {
            assert!(!contents.contains("{{"));
        }
    }

    #[test]
    fn a_scaffolded_form_is_doctor_clean() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pubspec.yaml"), "name: app\n").unwrap();
        // The app's one write boundary, as `skies g client` writes it (SKYFL027 is package-wide).
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(
            dir.path().join("lib/mutations.dart"),
            include_str!("../../templates/flutter/client/mutations.dart"),
        )
        .unwrap();
        scaffold(dir.path(), "wallet-transfer", FeatureKind::Form).unwrap();

        let findings = crate::flutter::rules::diagnose(dir.path()).unwrap();

        assert!(findings.is_empty(), "{findings:?}");
    }
}
