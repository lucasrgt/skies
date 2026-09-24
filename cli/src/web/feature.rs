//! `skies g feature` for a React web package: the ViewModel + View + i18n unit, in one of two kinds.
//!
//! `list` (the default) is the blessed `items` shape: the list slice's page folded into `AsyncState` and rendered
//! through `<Resource>`. `form` is the blessed `deposit` shape: a react-hook-form ViewModel whose submit goes through
//! `submitOrReveal` into a mutation, with pending, error, and success surfaces. List stays the default because a
//! module's first screen is usually the read of what it holds; a command screen asks for `--kind form`.
//!
//! Both read the backend's OpenAPI contract (the one `skies g client` generates from) so they bind to what orval
//! generates: the list to its slice's page and row type, the form to its command's inputs. A form needs its fields,
//! so without a contract it takes `--fields` or stops; a list without a contract falls back to a placeholder row.
//! The unit lands in a kebab-case folder named after the feature (`create-product/`), which is also its i18n
//! namespace, and imports only what a `skies g web-app` package provides (`@/ui`, `@/i18n`, `@/client.gen/<api>`).
//! Tests are not scaffolded: in Skies 5 a feature's evidence is the E2E in its spec folder, written against its
//! failure modes before the code, not a colocated test generated after it.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;

use super::form_fields::{self, Field, ListShape};
use super::names::{camel, kebab, pascal, singular};
use super::openapi::Document;
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
    // A read screen: the list slice's page as `AsyncState`, rendered through `<Resource>` with an empty state.
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
    /// The folder and the i18n namespace: `bookings`, `create-product`.
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
        let lower = kebab(&plural);
        Ok(FeatureNames {
            plural,
            collection,
            entity,
            lower,
        })
    }
}

/// The list slice a list feature reads: `Products` → `ListProducts`, the plural name `g crud` gives its list. A
/// contract that only has the singular name (`ListProduct`, from crud before the plural naming) is still honored.
pub fn list_slice(names: &FeatureNames, doc: Option<&Document>) -> String {
    let plural = format!("List{}", names.plural);
    let singular = format!("List{}", names.entity);
    match doc {
        Some(doc) if doc.operation(&plural).is_none() && doc.operation(&singular).is_some() => singular,
        _ => plural,
    }
}

/// What the templates need beyond the names.
pub enum Shape {
    List { slice: String, rows: Option<ListShape> },
    Form { fields: Vec<Field>, variables: String },
}

/// Renders the unit as `(file name, contents)` pairs, in the order they are reported. `client` is the
/// `@/client.gen/<client>` module the ViewModel imports its hook from; `locales` are the catalog's exports.
pub fn render_feature(
    names: &FeatureNames,
    shape: &Shape,
    client: &str,
    locales: &[String],
) -> Result<Vec<(String, String)>> {
    let title = humanize(&names.plural);
    let (view_model, view, i18n, ctx) = match shape {
        Shape::List { slice, rows } => {
            let collection = rows.as_ref().map_or(&names.collection, |r| &r.collection);
            let ctx = context! {
                plural => names.plural, entity => names.entity, lower => names.lower, collection, client, locales,
                slice, title,
                row => rows.as_ref().map(|r| r.row.clone()),
                display => rows.as_ref().map_or("name".to_string(), |r| r.display.clone()),
                key => rows.as_ref().map_or(Some("id".to_string()), |r| r.key.clone()),
            };
            (LIST_VIEW_MODEL, LIST_VIEW, LIST_I18N, ctx)
        }
        Shape::Form { fields, variables } => {
            let fields: Vec<Field> = fields
                .iter()
                .map(|field| Field {
                    rule: field
                        .rule
                        .replace("{msg}", &format!("i18n.t(\"{}:errors.{}\")", names.lower, field.name)),
                    ..field.clone()
                })
                .collect();
            let ctx = context! {
                name => names.plural, lower => names.lower, client, locales, fields, variables, title,
            };
            (FORM_VIEW_MODEL, FORM_VIEW, FORM_I18N, ctx)
        }
    };
    Ok(vec![
        (format!("{}.viewModel.ts", names.plural), render(view_model, &ctx)?),
        (format!("{}.view.tsx", names.plural), render(view, &ctx)?),
        (format!("{}.i18n.ts", names.lower), render(i18n, &ctx)?),
    ])
}

/// `CreateProduct` → `Create product`.
fn humanize(pascal: &str) -> String {
    let words = kebab(pascal).replace('-', " ");
    let mut chars = words.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_ascii_uppercase().to_string() + chars.as_str())
}

/// The mutation's variables: path parameters by name beside the body as `data`, the shape orval generates.
pub fn variables(fields: &[Field], has_body: bool) -> String {
    let path: Vec<String> = fields
        .iter()
        .filter(|f| f.in_path)
        .map(|f| format!("{}: {}", f.name, f.value))
        .collect();
    let body: Vec<String> = fields
        .iter()
        .filter(|f| !f.in_path)
        .map(|f| format!("{}: {}", f.name, f.value))
        .collect();
    let mut parts = path;
    if has_body || !body.is_empty() {
        parts.push(format!("data: {{ {} }}", body.join(", ")));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{{ {} }}", parts.join(", "))
    }
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

/// The backend contract for `package`, or why there is none.
fn load_contract(package: &Path) -> std::result::Result<(Document, PathBuf), String> {
    let found = contract::for_package(package).map_err(|error| format!("{error:#}"))?;
    let doc = Document::load(&found.path).map_err(|error| format!("{error:#}"))?;
    Ok((doc, found.path))
}

/// The form's inputs: `--fields` when given, else the command's operation in the contract.
fn form_shape(
    names: &FeatureNames,
    fields: Option<&str>,
    contract: &Result<(Document, PathBuf), String>,
) -> Result<Shape> {
    let slice = &names.plural;
    if let Some(spec) = fields {
        let fields = form_fields::parse(spec)?;
        let variables = variables(&fields, true);
        return Ok(Shape::Form { fields, variables });
    }
    let (doc, path) = match contract {
        Ok(found) => found,
        Err(reason) => bail!(
            "a form's fields come from the backend's OpenAPI contract, and none was found ({reason}). Build the \
             backend (`dotnet build` writes the contract), or pass the fields: --fields name:string,price:number \
             (nothing was written)"
        ),
    };
    let Some(operation) = doc.operation(slice) else {
        bail!(
            "{} has no `{slice}` operation (a slice mapped with `.WithName(nameof({slice}))`). Operations: {}. \
             Generate the slice and rebuild the backend, or pass --fields (nothing was written)",
            path.display(),
            doc.operation_ids().join(", ")
        );
    };
    let fields = form_fields::from_operation(doc, &operation, slice)?;
    let variables = variables(&fields, operation.body().is_some());
    Ok(Shape::Form { fields, variables })
}

pub fn scaffold(package: &Path, name: &str, kind: FeatureKind, fields: Option<&str>) -> Result<u8> {
    let names = FeatureNames::derive(name)?;
    if fields.is_some() && kind == FeatureKind::List {
        bail!("--fields describes a form's inputs; pass it with --kind form");
    }
    let dir = feature_dir(package, &names);
    let client = client_module(package);
    let locales = app_locales(package)?;
    let contract = load_contract(package);
    let doc = contract.as_ref().ok().map(|(doc, _)| doc);
    let shape = match kind {
        FeatureKind::List => {
            let slice = list_slice(&names, doc);
            let rows = doc
                .and_then(|doc| Some((doc, doc.operation(&slice)?)))
                .and_then(|(doc, op)| form_fields::list_shape(doc, &op));
            Shape::List { slice, rows }
        }
        FeatureKind::Form => form_shape(&names, fields, &contract)?,
    };
    let files: Vec<(PathBuf, String)> = render_feature(&names, &shape, &client, &locales)?
        .into_iter()
        .map(|(file, contents)| (dir.join(file), contents))
        .collect();
    write_new(&files)?;
    let slice = match &shape {
        Shape::List { slice, .. } => slice.clone(),
        Shape::Form { .. } => names.plural.clone(),
    };
    let unverified = match (&contract, &shape) {
        (Ok((doc, _)), _) if doc.operation(&slice).is_some() => String::new(),
        (Ok(_), _) => {
            format!(" The contract has no `{slice}` operation yet: generate the slice and rebuild the backend.")
        }
        (Err(_), Shape::List { .. }) => " No contract was found, so the row type is a placeholder to replace.".into(),
        (Err(_), Shape::Form { .. }) => " No contract was found, so the fields are the ones --fields named.".into(),
    };
    println!(
        "\nnext: the ViewModel imports `use{slice}` from @/client.gen/{client}, the hook `skies g client` generates \
         for the `{slice}` slice.{unverified} Run `skies g client` and `skies i18n`, then give the View a route \
         (src/routes/router.ts in a `skies g web-app` package)."
    );
    Ok(0)
}

#[cfg(test)]
#[path = "feature_tests.rs"]
mod tests;
