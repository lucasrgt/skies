//! `skies g feature` for a React web package: the ViewModel + View + i18n unit, in one of two kinds.
//!
//! `list` (the default) is the blessed `items` shape: a read hook folded into `AsyncState` and rendered through
//! `<Resource>`. `form` is the blessed `deposit` shape: a react-hook-form ViewModel whose submit goes through
//! `submitOrReveal` into a mutation, with pending, error, and success surfaces. List stays the default because a
//! module's first screen is usually the read of what it holds, and scripts written against the one-kind generator
//! keep their meaning; a command screen asks for `--kind form`. Both pass the SKYFE rules and typecheck against the
//! client `skies g client` generates (the form for a slice with the `g slice` scaffold's `Id` input, until its fields
//! are made the slice's own). Tests are not scaffolded: in Skies 5 a feature's evidence is the E2E in its
//! spec folder, written against its failure modes before the code, not a colocated test generated after it.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;

use super::names::{camel, pascal, singular};
use super::scaffold::{render, write_new};
use super::{contract, i18n};

const LIST_VIEW_MODEL: &str = include_str!("../../templates/react/feature/list/viewModel.ts");
const LIST_VIEW: &str = include_str!("../../templates/react/feature/list/view.tsx");
const LIST_I18N: &str = include_str!("../../templates/react/feature/list/i18n.ts");
const FORM_VIEW_MODEL: &str = include_str!("../../templates/react/feature/form/viewModel.ts");
const FORM_VIEW: &str = include_str!("../../templates/react/feature/form/view.tsx");
const FORM_I18N: &str = include_str!("../../templates/react/feature/form/i18n.ts");

/// What a feature screen does: read a collection, or send one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum FeatureKind {
    // A read screen: the `List<Name>` query as `AsyncState`, rendered through `<Resource>` with an empty state.
    #[default]
    List,
    // A command screen: a form ViewModel submitting the `<Name>` mutation, with pending, error, and success states.
    Form,
}

/// The names one feature name fans out into.
#[derive(Debug, PartialEq)]
pub struct FeatureNames {
    /// Component and hook stem: `Bookings`.
    pub plural: String,
    /// The resource field on the model state: `bookings`.
    pub collection: String,
    /// The row type: `Booking`.
    pub entity: String,
    /// The i18n namespace and the folder: `bookings`.
    pub lower: String,
}

impl FeatureNames {
    pub fn derive(name: &str) -> Result<FeatureNames> {
        let plural = pascal(name);
        if plural.is_empty() || !plural.starts_with(|c: char| c.is_ascii_alphabetic()) {
            bail!("'{name}' is not a usable feature name; start it with a letter");
        }
        let collection = camel(&plural);
        let entity = pascal(&singular(&camel(name)));
        let lower = collection.to_ascii_lowercase();
        Ok(FeatureNames {
            plural,
            collection,
            entity,
            lower,
        })
    }
}

/// Renders the unit as `(file name, contents)` pairs, in the order they are reported. `client` is the
/// `@/client.gen/<client>` module the ViewModel imports its hook from; `locales` are the catalog's exports.
pub fn render_feature(
    names: &FeatureNames,
    kind: FeatureKind,
    client: &str,
    locales: &[String],
) -> Result<Vec<(String, String)>> {
    let ctx = context! {
        plural => names.plural,
        name => names.plural,
        collection => names.collection,
        entity => names.entity,
        lower => names.lower,
        client,
        locales,
    };
    let (view_model, view, i18n) = match kind {
        FeatureKind::List => (LIST_VIEW_MODEL, LIST_VIEW, LIST_I18N),
        FeatureKind::Form => (FORM_VIEW_MODEL, FORM_VIEW, FORM_I18N),
    };
    Ok(vec![
        (format!("{}.viewModel.ts", names.plural), render(view_model, &ctx)?),
        (format!("{}.view.tsx", names.plural), render(view, &ctx)?),
        (format!("{}.i18n.ts", names.lower), render(i18n, &ctx)?),
    ])
}

/// The folder that holds the package's sources: `src/` when it has one, else its root.
fn source_root(package: &Path) -> PathBuf {
    let src = package.join("src");
    if src.is_dir() { src } else { package.to_path_buf() }
}

/// Where a feature lands: `src/<feature>/` when the package has a `src/`, else `<feature>/` at its root.
pub fn feature_dir(package: &Path, names: &FeatureNames) -> PathBuf {
    source_root(package).join(&names.lower)
}

/// The `client.gen` module a ViewModel imports: the backend's client name when that module exists or none does yet
/// (so the import is right before `skies g client` runs), else the package's only generated module. The feature's
/// own name is never the guess: one client module serves every feature of a package.
pub fn client_module(package: &Path) -> String {
    let mut modules: Vec<String> = std::fs::read_dir(source_root(package).join("client.gen"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "ts"))
                .filter_map(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
                .filter(|stem| !stem.ends_with(".d"))
                .collect()
        })
        .unwrap_or_default();
    modules.sort();
    let expected = contract::client_name_for(package).ok();
    match (expected, modules.as_slice()) {
        (Some(expected), _) if modules.contains(&expected) => expected,
        (_, [only]) => only.clone(),
        (Some(expected), _) => expected,
        (None, [first, ..]) => first.clone(),
        (None, []) => "api".to_string(),
    }
}

/// The package's locale set: the exports of its first existing `*.i18n.ts` catalog (by path), so a new feature
/// declares exactly the locales every other catalog does. A package with no catalog yet gets a single `en`; adding a
/// locale later means adding one export to each catalog.
pub fn app_locales(package: &Path) -> Result<Vec<String>> {
    let root = source_root(package);
    let mut catalogs = Vec::new();
    if root.is_dir() {
        i18n::find_catalogs(&root, &mut catalogs)?;
    }
    catalogs.sort();
    for catalog in catalogs {
        let locales = i18n::catalog_locales(&std::fs::read_to_string(&catalog)?);
        if !locales.is_empty() {
            return Ok(locales);
        }
    }
    Ok(vec!["en".to_string()])
}

pub fn scaffold(package: &Path, name: &str, kind: FeatureKind) -> Result<u8> {
    let names = FeatureNames::derive(name)?;
    let dir = feature_dir(package, &names);
    let client = client_module(package);
    let locales = app_locales(package)?;
    let files: Vec<(PathBuf, String)> = render_feature(&names, kind, &client, &locales)?
        .into_iter()
        .map(|(file, contents)| (dir.join(file), contents))
        .collect();
    write_new(&files)?;
    let slice = match kind {
        FeatureKind::List => format!("List{}", names.plural),
        FeatureKind::Form => names.plural.clone(),
    };
    println!(
        "\nnext: the ViewModel imports `use{slice}` from @/client.gen/{client}: the hook `skies g client` generates \
         for the `{slice}` slice (`.WithName(nameof({slice}))`). Generate the slice if it does not exist, run \
         `skies g client`, refine the fields and copy, then `skies i18n`."
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locales(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn rendered(name: &str) -> Vec<(String, String)> {
        let set = locales(&["ptBR", "esES", "enUS"]);
        render_feature(&FeatureNames::derive(name).unwrap(), FeatureKind::List, "shop", &set).unwrap()
    }

    fn form(name: &str) -> Vec<(String, String)> {
        let set = locales(&["ptBR", "esES", "enUS"]);
        render_feature(&FeatureNames::derive(name).unwrap(), FeatureKind::Form, "shop", &set).unwrap()
    }

    /// The keys of one locale block of a rendered i18n module.
    fn keys(i18n: &str, locale: &str) -> Vec<String> {
        let block = i18n.split(&format!("export const {locale} = {{")).nth(1).unwrap();
        let block = &block[..block.find("} as const").unwrap()];
        block
            .lines()
            .filter_map(|line| line.trim().split(':').next().map(str::to_string))
            .filter(|key| !key.is_empty())
            .collect()
    }

    #[test]
    fn derives_names_from_a_plural_feature_name() {
        let names = FeatureNames::derive("user-profiles").unwrap();
        assert_eq!(names.plural, "UserProfiles");
        assert_eq!(names.collection, "userProfiles");
        assert_eq!(names.entity, "UserProfile");
        assert_eq!(names.lower, "userprofiles");
    }

    #[test]
    fn emits_the_three_files_of_the_unit_and_no_tests() {
        let files: Vec<String> = rendered("bookings").into_iter().map(|(name, _)| name).collect();
        assert_eq!(
            files,
            ["Bookings.viewModel.ts", "Bookings.view.tsx", "bookings.i18n.ts"]
        );
    }

    #[test]
    fn wires_the_spine_without_proof_ceremony() {
        let files = rendered("bookings");
        let (view_model, view, i18n) = (&files[0].1, &files[1].1, &files[2].1);

        assert!(view_model.contains("AsyncState<Booking[]>"));
        assert!(view_model.contains("import { useListBookings } from \"@/client.gen/shop\";"));
        assert!(view_model.contains("i18n.t(\"bookings:error\")"));
        assert!(
            view_model.contains("data: query.data?.bookings?.items,"),
            "the List slice returns a page"
        );
        assert!(view.contains("<Resource"));
        assert!(view.contains("{(bookings) => <BookingsList bookings={bookings} />}"));
        assert!(view.contains("{bookings.map((item) => ("));
        for locale in ["ptBR", "esES", "enUS"] {
            assert!(i18n.contains(&format!("export const {locale}")));
        }
        for (_, contents) in &files {
            for ceremony in ["@verify", "@avp", "@e2e", "defineVerification", "{{", "{%"] {
                assert!(!contents.contains(ceremony), "{ceremony} leaked into the scaffold");
            }
        }
    }

    #[test]
    fn a_form_is_the_command_recipe() {
        let files = form("transfer");
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            ["Transfer.viewModel.ts", "Transfer.view.tsx", "transfer.i18n.ts"]
        );
        let (view_model, view, i18n) = (&files[0].1, &files[1].1, &files[2].1);

        assert!(view_model.contains("import { useTransfer } from \"@/client.gen/shop\";"));
        assert!(view_model.contains("const form = useForm<TransferForm>({"));
        assert!(view_model.contains("const submit = submitOrReveal(\n    form.handleSubmit,"));
        assert!(view_model.contains("mutation.mutate({ data: { id: values.id } })"));
        assert!(view_model.contains("submitting: mutation.isPending,"));
        assert!(view_model.contains("submitError: mutation.isError ? i18n.t(\"transfer:errors.submit\") : null,"));
        assert!(view_model.contains("completed: mutation.isSuccess,"));
        assert!(!view_model.contains("useList") && !view_model.contains("AsyncState"));
        assert!(view.contains("render={({ field, fieldState }) => ("));
        assert!(view.contains("error={fieldState.error?.message}"));
        assert!(view.contains("<Text role=\"label\" tone=\"danger\" alert>"));
        assert!(view.contains("loading={submitting}"));
        assert!(!view.contains("EmptyState") && !view.contains("client.gen"));
        assert_eq!(keys(i18n, "ptBR"), keys(i18n, "enUS"));
        assert_eq!(keys(i18n, "esES"), keys(i18n, "enUS"));
        assert!(i18n.contains("  title: \"Transfer\",\n"));
        for (_, contents) in &files {
            assert!(!contents.contains("{{") && !contents.contains("{%"));
        }
    }

    #[test]
    fn the_locale_set_is_the_packages_and_defaults_to_english() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/b")).unwrap();
        assert_eq!(app_locales(dir.path()).unwrap(), ["en"]);

        scaffold(dir.path(), "Items", FeatureKind::List).unwrap();
        let i18n = std::fs::read_to_string(dir.path().join("src/items/items.i18n.ts")).unwrap();
        assert!(i18n.contains("export const en = {"));
        assert_eq!(i18n.matches("export const").count(), 1);
        assert!(i18n.ends_with("} as const;\n") && !i18n.ends_with("\n\n"));

        std::fs::remove_file(dir.path().join("src/items/items.i18n.ts")).unwrap();
        std::fs::write(
            dir.path().join("src/b/a.i18n.ts"),
            "export const frFR = {} as const;\nexport const deDE = {} as const;\n",
        )
        .unwrap();
        assert_eq!(app_locales(dir.path()).unwrap(), ["frFR", "deDE"]);

        scaffold(dir.path(), "Transfer", FeatureKind::Form).unwrap();
        let i18n = std::fs::read_to_string(dir.path().join("src/transfer/transfer.i18n.ts")).unwrap();
        assert_eq!(keys(&i18n, "frFR"), keys(&i18n, "deDE"));
        assert!(!i18n.contains("enUS") && !i18n.contains("ptBR"));
    }

    #[test]
    fn scaffolds_cite_no_rule_ids() {
        for (_, contents) in rendered("bookings").into_iter().chain(form("transfer")) {
            assert!(
                !contents.contains("SKY"),
                "a rule id leaked into the scaffold:\n{contents}"
            );
        }
    }

    #[test]
    fn rejects_a_name_without_letters() {
        assert!(FeatureNames::derive("--").is_err());
        assert!(FeatureNames::derive("9lives").is_err());
    }

    #[test]
    fn writes_under_src_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();

        scaffold(dir.path(), "Profile", FeatureKind::List).unwrap();

        assert!(dir.path().join("src/profile/Profile.viewModel.ts").is_file());
        assert!(scaffold(dir.path(), "Profile", FeatureKind::Form).is_err());
    }

    #[test]
    fn the_client_module_is_the_packages_not_the_features() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Skies.toml"),
            "[workspace]\nname = \"s\"\n[products.app]\nbackend = \"api/Sample.Api\"\nfrontend = \"web\"\n",
        )
        .unwrap();
        let web = dir.path().join("web");
        std::fs::create_dir_all(web.join("src/client.gen/model")).unwrap();
        let web = web.canonicalize().unwrap();
        assert_eq!(client_module(&web), "sample", "the name g client will write");

        std::fs::write(web.join("src/client.gen/other.ts"), "").unwrap();
        std::fs::write(web.join("src/client.gen/sample.ts"), "").unwrap();
        assert_eq!(client_module(&web), "sample");

        let lone = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(lone.path().join("src/client.gen")).unwrap();
        std::fs::write(lone.path().join("src/client.gen/shop.ts"), "").unwrap();
        assert_eq!(client_module(lone.path()), "shop");
    }
}
