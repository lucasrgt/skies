//! `skies g feature` for React: the ViewModel + View + i18n unit.
//!
//! The emitted unit is the blessed `items` shape with names substituted, so it passes the SKYFE rules and
//! typechecks by construction. Tests are not scaffolded: in Skies 5 a feature's evidence is the E2E in its spec
//! folder, written against its failure modes before the code, not a colocated test generated after it.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use minijinja::context;

use super::names::{camel, pascal, singular};
use super::scaffold::{render, write_new};

const VIEW_MODEL: &str = include_str!("../../templates/react/feature/viewModel.ts");
const VIEW: &str = include_str!("../../templates/react/feature/view.tsx");
const I18N: &str = include_str!("../../templates/react/feature/i18n.ts");

/// The names one feature name fans out into.
#[derive(Debug, PartialEq)]
pub struct FeatureNames {
    /// Component and hook stem: `Bookings`.
    pub plural: String,
    /// The resource field on the model state: `bookings`.
    pub collection: String,
    /// The row type: `Booking`.
    pub entity: String,
    /// The i18n namespace, the folder, and the `client.gen` module: `bookings`.
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

/// Renders the unit as `(file name, contents)` pairs, in the order they are reported.
pub fn render_feature(names: &FeatureNames) -> Result<Vec<(String, String)>> {
    let ctx = context! {
        plural => names.plural,
        collection => names.collection,
        entity => names.entity,
        lower => names.lower,
    };
    Ok(vec![
        (
            format!("{}.viewModel.ts", names.plural),
            render(VIEW_MODEL, &ctx)?,
        ),
        (format!("{}.view.tsx", names.plural), render(VIEW, &ctx)?),
        (format!("{}.i18n.ts", names.lower), I18N.to_string()),
    ])
}

/// Where a feature lands: `src/<feature>/` when the package has a `src/`, else `<feature>/` at its root.
pub fn feature_dir(package: &Path, names: &FeatureNames) -> PathBuf {
    let src = package.join("src");
    if src.is_dir() {
        src.join(&names.lower)
    } else {
        package.join(&names.lower)
    }
}

pub fn scaffold(package: &Path, name: &str) -> Result<u8> {
    let names = FeatureNames::derive(name)?;
    let dir = feature_dir(package, &names);
    let files: Vec<(PathBuf, String)> = render_feature(&names)?
        .into_iter()
        .map(|(file, contents)| (dir.join(file), contents))
        .collect();
    write_new(&files)?;
    println!(
        "\nnext: generate the `list_{}` hook with `skies g client`, refine the entity and copy, then `skies i18n`.",
        names.lower
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(name: &str) -> Vec<(String, String)> {
        render_feature(&FeatureNames::derive(name).unwrap()).unwrap()
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
        let files: Vec<String> = rendered("bookings")
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(
            files,
            [
                "Bookings.viewModel.ts",
                "Bookings.view.tsx",
                "bookings.i18n.ts"
            ]
        );
    }

    #[test]
    fn wires_the_spine_without_proof_ceremony() {
        let files = rendered("bookings");
        let (view_model, view, i18n) = (&files[0].1, &files[1].1, &files[2].1);

        assert!(view_model.contains("AsyncState<Booking[]>"));
        assert!(view_model.contains("import { useListBookings } from \"@/client.gen/bookings\";"));
        assert!(view_model.contains("i18n.t(\"bookings:error\")"));
        assert!(view.contains("<Resource"));
        assert!(view.contains("{(bookings) => <BookingsList bookings={bookings} />}"));
        assert!(view.contains("{bookings.map((item) => ("));
        for locale in ["ptBR", "esES", "enUS"] {
            assert!(i18n.contains(&format!("export const {locale}")));
        }
        for (_, contents) in &files {
            for ceremony in ["@verify", "@avp", "@e2e", "defineVerification", "{{", "{%"] {
                assert!(
                    !contents.contains(ceremony),
                    "{ceremony} leaked into the scaffold"
                );
            }
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

        scaffold(dir.path(), "Profile").unwrap();

        assert!(
            dir.path()
                .join("src/profile/Profile.viewModel.ts")
                .is_file()
        );
        assert!(scaffold(dir.path(), "Profile").is_err());
    }
}
