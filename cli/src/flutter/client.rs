//! `skies g client` for Flutter: a dart-dio package from the backend contract, plus the hand-owned seams.
//!
//! The generator is stock OpenAPI Generator (`dart-dio`, `built_value`), pinned by the embedded
//! `openapitools.json` and run through `@openapitools/openapi-generator-cli`. The package is generated into a
//! scratch directory, built and analyzed there, and only then swapped in, so a failed run never leaves a
//! half-written client. A directory is replaced only if it carries the marker a previous run wrote.
//!
//! `--input`, `--output`, `--name`, and `--version` take the place of the Skies 4 `skies-flutter-client` flags one
//! to one, for apps whose packages keep their contract or their generated package somewhere other than the
//! defaults. With `--input` or `--output`, only the package is generated, as the 4.x tool did: the app already owns
//! its seams and its pubspec entry, and a guessed path dependency would be wrong.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use minijinja::context;

use super::names::{is_package_name, pascal};
use super::projection::project_app_client;
use crate::web::contract;
use crate::web::scaffold::{render, run, tree_hash, write_missing};

const OPENAPITOOLS: &str = include_str!("../../templates/flutter/openapitools.json");
const CLIENT_SEAM: &str = include_str!("../../templates/flutter/client/skies_client.dart");
const SESSION_SEAM: &str = include_str!("../../templates/flutter/client/session.dart");
const MUTATION_SEAM: &str = include_str!("../../templates/flutter/client/mutations.dart");

/// Overrides of the defaults read from `Skies.toml`. Relative paths resolve against the current directory, as the
/// 4.x `skies-flutter-client` resolved them, so a package script keeps working unchanged.
#[derive(Debug, Default, Clone)]
pub struct ClientOptions {
    /// The OpenAPI document; defaults to the contract of the backend that declares this package.
    pub input: Option<PathBuf>,
    /// The generated package directory; defaults to `packages/<name>` inside the app.
    pub output: Option<PathBuf>,
    /// The Dart package name; defaults to `<backend>_api`.
    pub name: Option<String>,
    /// The generated package's pub version; defaults to 0.1.0.
    pub version: Option<String>,
}

/// The npm package that downloads and runs the generator jar pinned in `openapitools.json`.
const GENERATOR_CLI: &str = "@openapitools/openapi-generator-cli@2.41.0";
/// The file that marks a generated client package; the doctor skips such a package whole.
pub(crate) const MARKER: &str = ".skies-generated-client";

/// The generator arguments: dart-dio with built_value, no docs or tests (the app owns its tests).
pub fn generator_arguments(input: &str, output: &str, name: &str, version: &str) -> Result<Vec<String>> {
    if !is_package_name(name) {
        bail!("'{name}' must be a lowercase Dart package identifier");
    }
    // The version lands in the same comma-separated property list as the name, so it must not carry separators.
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)) {
        bail!("'{version}' is not a pub version");
    }
    Ok([
        "generate",
        "-g",
        "dart-dio",
        "-i",
        input,
        "-o",
        output,
        "--additional-properties",
        &format!("pubName={name},pubVersion={version},pubPublishTo=none,serializationLibrary=built_value"),
        "--global-property",
        "apiDocs=false,modelDocs=false,apiTests=false,modelTests=false",
    ]
    .map(String::from)
    .to_vec())
}

/// The hand-owned seams under `lib/`: the HTTP client, the session, and the mutation boundary.
pub fn render_seams(package_name: &str, client_class: &str) -> Result<Vec<(&'static str, String)>> {
    Ok(vec![
        (
            "skies_client.dart",
            render(CLIENT_SEAM, context! { package => package_name, class => client_class })?,
        ),
        ("session.dart", SESSION_SEAM.to_string()),
        ("mutations.dart", MUTATION_SEAM.to_string()),
    ])
}

pub fn generate(package: &Path, options: &ClientOptions) -> Result<u8> {
    let cwd = std::env::current_dir()?;
    let (input, default_name) = match &options.input {
        Some(input) => {
            let input = cwd.join(input);
            let stem = input.file_stem().map(|stem| stem.to_string_lossy().into_owned());
            let name = contract::client_name(stem.as_deref().unwrap_or_default());
            (input, name)
        }
        None => {
            let contract = contract::for_package(package)?;
            (contract.path, contract.name)
        }
    };
    if !input.is_file() {
        bail!("contract not found: {}", input.display());
    }
    let name = options.name.clone().unwrap_or_else(|| format!("{default_name}_api"));
    let version = options.version.as_deref().unwrap_or("0.1.0");
    let output = match &options.output {
        Some(output) => cwd.join(output),
        None => package.join("packages").join(&name),
    };
    let changed = generate_package(&input, &output, &name, version)?;
    println!(
        "{} {} from {}",
        output.display(),
        if changed { "updated" } else { "unchanged" },
        input.display()
    );
    if options.input.is_some() || options.output.is_some() {
        return Ok(0);
    }

    let class = pascal(&name);

    let seams: Vec<(PathBuf, String)> = render_seams(&name, &class)?
        .into_iter()
        .map(|(file, contents)| (package.join("lib").join(file), contents))
        .collect();
    write_missing(&seams)?;
    add_dependencies(package, &name)?;
    Ok(0)
}

/// Generates, verifies, and publishes the package. Returns whether its `lib/` changed.
fn generate_package(contract: &Path, output: &Path, name: &str, version: &str) -> Result<bool> {
    let parent = output.parent().context("output has a parent")?;
    std::fs::create_dir_all(parent)?;
    if output.exists() && !output.join(MARKER).exists() {
        bail!(
            "refusing to replace {}: {MARKER} is missing, so it was not generated by skies",
            output.display()
        );
    }
    let scratch = tempfile::Builder::new()
        .prefix(".skies-flutter-client-")
        .tempdir_in(parent)?;
    let projected = scratch.path().join("app-client.openapi.json");
    let generated = scratch.path().join("generated");

    let document = serde_json::from_str(&std::fs::read_to_string(contract)?)
        .with_context(|| format!("parsing {}", contract.display()))?;
    std::fs::write(
        &projected,
        format!("{}\n", serde_json::to_string_pretty(&project_app_client(&document)?)?),
    )?;
    std::fs::write(scratch.path().join("openapitools.json"), OPENAPITOOLS)?;

    let args = generator_arguments("app-client.openapi.json", "generated", name, version)?;
    let mut npx_args = vec!["--yes", GENERATOR_CLI];
    npx_args.extend(args.iter().map(String::as_str));
    run(if cfg!(windows) { "npx.cmd" } else { "npx" }, &npx_args, scratch.path())?;

    let dart = std::env::var("DART_BIN").unwrap_or_else(|_| "dart".into());
    run(&dart, &["pub", "get"], &generated)?;
    run(
        &dart,
        &["run", "build_runner", "build", "--delete-conflicting-outputs"],
        &generated,
    )?;
    run(&dart, &["format", "lib"], &generated)?;
    silence_generator_imports(&generated.join("analysis_options.yaml"))?;
    run(&dart, &["analyze", "--no-fatal-warnings"], &generated)?;
    std::fs::write(generated.join(MARKER), "generated by skies; replace as a unit\n")?;

    let before = tree_hash(&output.join("lib"));
    publish(&generated, output)?;
    Ok(before != tree_hash(&output.join("lib")))
}

/// dart-dio imports every model an API file might need and not only the ones it uses, so the app's
/// `flutter analyze` (which also walks `packages/`) would report warnings nobody can fix: the package is replaced
/// as a unit on every run. The generated package's own analyzer config ignores exactly that lint.
fn silence_generator_imports(options: &Path) -> Result<()> {
    let text = std::fs::read_to_string(options).unwrap_or_default();
    let updated = match text.find("  errors:\n") {
        Some(at) => {
            format!(
                "{}  errors:\n    unused_import: ignore\n{}",
                &text[..at],
                &text[at + "  errors:\n".len()..]
            )
        }
        None if text.contains("analyzer:\n") => {
            text.replacen("analyzer:\n", "analyzer:\n  errors:\n    unused_import: ignore\n", 1)
        }
        None => format!("{text}analyzer:\n  errors:\n    unused_import: ignore\n"),
    };
    std::fs::write(options, updated).with_context(|| format!("writing {}", options.display()))
}

/// Swaps the verified package in, restoring the previous one if the final rename fails.
fn publish(generated: &Path, output: &Path) -> Result<()> {
    if !output.exists() {
        return std::fs::rename(generated, output).with_context(|| format!("moving into {}", output.display()));
    }
    let backup = output.with_extension("skies-backup");
    std::fs::rename(output, &backup)?;
    if let Err(error) = std::fs::rename(generated, output) {
        std::fs::rename(&backup, output)?;
        return Err(error).with_context(|| format!("moving into {}", output.display()));
    }
    std::fs::remove_dir_all(&backup)?;
    Ok(())
}

/// Adds the generated package (by path) and `dio` to the app when its pubspec does not list them yet.
fn add_dependencies(package: &Path, name: &str) -> Result<()> {
    let pubspec = std::fs::read_to_string(package.join("pubspec.yaml"))?;
    let listed = |dependency: &str| {
        pubspec
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{dependency}:")))
    };
    let mut wanted = Vec::new();
    if !listed(name) {
        wanted.push(format!("{name}:{{\"path\":\"packages/{name}\"}}"));
    }
    if !listed("dio") {
        wanted.push("dio".to_string());
    }
    if wanted.is_empty() {
        return Ok(());
    }
    let flutter = std::env::var("FLUTTER_BIN").unwrap_or_else(|_| "flutter".into());
    let mut args = vec!["pub", "add"];
    args.extend(wanted.iter().map(String::as_str));
    run(&flutter, &args, package)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_dart_dio_built_value_and_package_metadata() {
        let args = generator_arguments("contract.json", "client.gen", "sample_api", "1.2.3").unwrap();
        assert_eq!(args[..5], ["generate", "-g", "dart-dio", "-i", "contract.json"]);
        assert!(args.contains(
            &"pubName=sample_api,pubVersion=1.2.3,pubPublishTo=none,serializationLibrary=built_value".into()
        ));
        assert!(args.contains(&"apiDocs=false,modelDocs=false,apiTests=false,modelTests=false".into()));
    }

    #[test]
    fn rejects_a_version_that_could_inject_generator_properties() {
        let error = generator_arguments("x", "y", "sample_api", "1.0.0,pubName=evil").unwrap_err();
        assert!(error.to_string().contains("version"));
    }

    #[test]
    fn rejects_a_package_name_that_could_inject_generator_properties() {
        let error = generator_arguments("x", "y", "sample,hide=true", "0.1.0").unwrap_err();
        assert!(error.to_string().contains("lowercase Dart package identifier"));
    }

    #[test]
    fn the_client_seam_owns_base_url_auth_and_error_mapping() {
        let seams = render_seams("sample_api", "SampleApi").unwrap();
        let client = &seams[0].1;
        assert!(client.contains("import 'package:sample_api/sample_api.dart';"));
        assert!(client.contains("Dio(BaseOptions(baseUrl: baseUrl))"));
        assert!(client.contains("List<Interceptor> interceptors"));
        assert!(client.contains("SkiesAuthInterceptor"));
        assert!(client.contains("executeSkiesRequest<T, ErrorBody>"));
        assert!(client.contains("standardSerializers.deserializeWith(ErrorBody.serializer, data)"));
        assert!(client.contains("late final SampleApi api"));
    }

    #[test]
    fn the_session_and_mutation_seams_are_app_owned_composition() {
        let seams = render_seams("sample_api", "SampleApi").unwrap();
        assert!(seams[1].1.contains("onIdentityChanged: clearIdentityCache"));
        assert!(seams[1].1.contains("onSessionChanged: resetSessionCache"));
        assert!(seams[2].1.contains("MutationBoundary"));
        assert!(seams[2].1.contains("invalidateQueries"));
    }

    #[test]
    fn the_generated_package_ignores_the_generator_s_unused_imports() {
        let dir = tempfile::tempdir().unwrap();
        let options = dir.path().join("analysis_options.yaml");
        std::fs::write(
            &options,
            "analyzer:\n  exclude:\n    - test/*.dart\n  errors:\n    deprecated: ignore\n",
        )
        .unwrap();

        silence_generator_imports(&options).unwrap();

        let text = std::fs::read_to_string(&options).unwrap();
        assert!(
            text.ends_with("  errors:\n    unused_import: ignore\n    deprecated: ignore\n"),
            "{text}"
        );
    }

    #[test]
    fn refuses_to_replace_a_directory_it_did_not_generate() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("packages/sample_api");
        std::fs::create_dir_all(&output).unwrap();
        let error = generate_package(Path::new("missing.json"), &output, "sample_api", "0.1.0").unwrap_err();
        assert!(error.to_string().contains(MARKER));
    }
}
