//! `skies` — the Skies convention CLI.
//!
//! Scaffolders emit plain, doctor-conformant code for .NET, React, and Flutter. `doctor` runs the architecture
//! analyzers each platform already owns. `spec` and `proof` record reproducible evidence that a feature works.
//! Nothing here is a gate: every command is something a person or an agent runs on purpose.

mod doctor;
mod dotnet;
mod flutter;
mod manifest;
mod migrate;
mod proof;
mod web;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skies", version, about = "Scaffold, check, and prove Skies applications.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new Skies application in ./<Name>.
    New {
        /// The application name; it becomes the solution and root namespace.
        name: String,
    },
    /// Generate code that follows the Skies conventions.
    #[command(subcommand, name = "g", alias = "generate")]
    Generate(Generate),
    /// Assemble per-feature i18n catalogs into the package's locale files.
    I18n {
        /// The frontend package directory (defaults to the current directory).
        #[arg(long)]
        package: Option<PathBuf>,
    },
    /// Run the architecture doctors: the declared repository root (SKYWS*), dotnet build (SKY*), eslint (SKYFE*),
    /// and the Flutter rules (SKYFL*).
    Doctor {
        /// Check only this package directory (a Flutter or React package, or a .NET project or its folder), so a
        /// package's own `lint` script can call the doctor (the root is not checked).
        #[arg(long)]
        package: Option<PathBuf>,
        /// Extra arguments forwarded to `dotnet build`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        build_args: Vec<String>,
    },
    /// Work with feature specs under .specs/.
    #[command(subcommand)]
    Spec(Spec),
    /// Record and check the evidence that a spec's failure modes are handled.
    #[command(subcommand)]
    Proof(Proof),
    /// Migrate an application to a new Skies major version.
    Migrate {
        /// The target major version. Only 5 is supported.
        version: u32,
        /// Show what would change without writing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Where a backend generator writes.
#[derive(clap::Args)]
pub struct Backend {
    /// The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project,
    /// else the backend Skies.toml declares (with several, the one holding the module).
    #[arg(long, value_name = "DIR")]
    pub project: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Generate {
    /// A module: <Name>Module.cs and a ctx.md skeleton for you to write, wired into the module registry.
    Module {
        name: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// A slice inside a module, mapped under the module's route group.
    Slice {
        module: String,
        name: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// A rich [Entity] with an EnsureValid invariant funnel.
    Entity {
        module: String,
        name: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// An always-valid [ValueObject] in BuildingBlocks.
    Vo {
        name: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// List/lookup/create/update/delete slices and a view record for an [Entity], its DbSet, and its Open/Update.
    Crud {
        module: String,
        entity: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// A SignalR hub for real-time fan-out.
    Hub {
        module: String,
        name: String,
        #[command(flatten)]
        backend: Backend,
    },
    /// The auth module: register, login, refresh, logout, me, sessions.
    Auth {
        /// Leave out multi-tenant scoping (the Tenancy/ files and the request tenant).
        #[arg(long)]
        skip_tenancy: bool,
        /// Leave out web-cookie refresh delivery; the refresh token travels in the response body only.
        #[arg(long)]
        skip_cookies: bool,
        #[command(flatten)]
        backend: Backend,
    },
    /// Phone verification by SMS code.
    #[command(name = "auth:otp")]
    AuthOtp {
        #[command(flatten)]
        backend: Backend,
    },
    /// Google sign-up and sign-in.
    #[command(name = "auth:oauth")]
    AuthOauth {
        #[command(flatten)]
        backend: Backend,
    },
    /// Email verification and password reset.
    #[command(name = "auth:email")]
    AuthEmail {
        #[command(flatten)]
        backend: Backend,
    },
    /// A frontend feature (ViewModel + View + i18n) in a React web or Flutter package.
    Feature {
        name: String,
        /// `list`: a read screen over the `List<Name>` query. `form`: a command screen that submits the `<Name>`
        /// mutation, with validation, pending, error, and success states.
        #[arg(long, value_enum, default_value_t = web::FeatureKind::List)]
        kind: web::FeatureKind,
        /// The frontend package directory (defaults to the current directory).
        #[arg(long)]
        package: Option<PathBuf>,
    },
    /// The typed API client for a React web or Flutter package, from the backend's OpenAPI contract.
    Client {
        /// The frontend package directory (defaults to the current directory).
        #[arg(long)]
        package: Option<PathBuf>,
        /// Flutter: the OpenAPI document (defaults to the backend contract `Skies.toml` points at).
        #[arg(long)]
        input: Option<PathBuf>,
        /// Flutter: the generated package directory (defaults to packages/<name>).
        #[arg(long)]
        output: Option<PathBuf>,
        /// Flutter: the generated Dart package name (defaults to <backend>_api).
        #[arg(long)]
        name: Option<String>,
        /// Flutter: the generated package's pub version (defaults to 0.1.0).
        #[arg(long)]
        version: Option<String>,
    },
    /// A Flutter application package wired to the Skies spine.
    #[command(name = "flutter-app")]
    FlutterApp {
        name: String,
        /// Where to create the package (defaults to ./<name>).
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum Spec {
    /// Create .specs/<id>-<slug>/ with a spec.md to fill in.
    New {
        slug: String,
        /// The runner from Skies.toml that executes this spec's e2e/ folder.
        #[arg(long)]
        runner: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum Proof {
    /// Run the spec's E2E against the red revision (must fail) and the working tree (must pass); write receipt.json.
    /// Notes (never fails) a touched module whose ctx.md was not revised in the same change.
    Record {
        /// The spec id or folder name.
        spec: String,
        /// The revision the failure modes must fail on. Defaults to HEAD plus the spec's red.patch when it has one,
        /// else the merge-base of HEAD with `[workspace] default_branch` from Skies.toml, else with the current
        /// branch's upstream (when it is another branch), else with origin/HEAD. The choice is printed.
        #[arg(long)]
        red: Option<String>,
        /// A patch applied to the red checkout before running, for specs written after the code.
        #[arg(long)]
        red_patch: Option<PathBuf>,
        /// After recording, rerun green for every other spec whose footprint overlaps this one's and name the ones
        /// that pass in the receipt's `verified_with`. Exits 1 if any of them fails; a spec without a receipt is
        /// reported as unrecorded and does not count.
        #[arg(long)]
        with_impacted: bool,
        /// Rerun red alone (with the spec's red.patch, --red-patch, or --red) and rewrite only the red half of an
        /// existing receipt, keeping green, the footprint, and green's evidence: the fix for a `red-rotted` spec.
        #[arg(long)]
        red_only: bool,
    },
    /// Run the spec's E2E once on the working tree and print each failure mode's pass or fail, with what the
    /// failing cases reported. Writes no receipt and no committed evidence; the report and output stay in the
    /// spec's gitignored evidence/raw/ for inspection. Exits 1 unless every mode passes.
    Run {
        /// The spec id or folder name.
        spec: String,
    },
    /// List receipts that are current, stale (their files changed), tampered (their evidence was edited), or
    /// unrecorded (no receipt yet), and red-rotted specs (red.patch no longer applies). Hashes only; runs no tests,
    /// and git only for a red.patch that changed, or whose files did, since the last check.
    Status,
    /// Show which specs a change reaches, from the receipts' footprints and spec.md `touches`: each spec with its
    /// failure modes and whether its receipt is current, then the ctx.md of every module the paths reach. With no
    /// paths, uses the files changed on this branch.
    Impact {
        /// Files or directories to look up, relative to the current directory.
        paths: Vec<PathBuf>,
        /// Also look up the files changed since <rev> (default: the merge-base with the default branch), including
        /// uncommitted and untracked files.
        #[arg(long, value_name = "REV", num_args = 0..=1, default_missing_value = "")]
        diff: Option<String>,
    },
    /// Rerun specs' green. A receipt that is current and still passes is left untouched ("verified (current,
    /// unchanged)"); a stale one gets fresh green evidence and footprint. Red is never rerun.
    Verify {
        /// Spec ids or folder names.
        specs: Vec<String>,
        /// Rerun every spec whose receipt is stale.
        #[arg(long)]
        stale: bool,
        /// Rerun every spec.
        #[arg(long)]
        all: bool,
        /// Rewrite the green evidence and footprint of current receipts too (and of tampered ones, whose red
        /// evidence keeps its recorded hashes).
        #[arg(long)]
        refresh: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::New { name } => dotnet::new_app(&name),
        Command::Generate(generate) => generate_command(generate),
        Command::I18n { package } => web::i18n(package.as_deref()),
        Command::Doctor { package, build_args } => doctor::run(&build_args, package.as_deref()),
        Command::Spec(Spec::New { slug, runner }) => proof::spec_new(&slug, runner.as_deref()),
        Command::Proof(Proof::Record {
            spec,
            red,
            red_patch,
            with_impacted,
            red_only,
        }) => proof::record(
            &spec,
            &proof::RecordOptions {
                red: red.as_deref(),
                red_patch: red_patch.as_deref(),
                with_impacted,
                red_only,
            },
        ),
        Command::Proof(Proof::Run { spec }) => proof::run(&spec),
        Command::Proof(Proof::Status) => proof::status(),
        Command::Proof(Proof::Impact { paths, diff }) => proof::impact(&paths, diff.as_deref()),
        Command::Proof(Proof::Verify {
            specs,
            stale,
            all,
            refresh,
        }) => proof::verify(&specs, stale, all, refresh),
        Command::Migrate { version, dry_run } => migrate::run(version, dry_run),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("skies: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn generate_command(generate: Generate) -> anyhow::Result<u8> {
    match generate {
        Generate::Feature {
            name,
            kind: feature_kind,
            package,
        } => frontend_package(package.as_deref(), |kind, dir| match kind {
            FrontendKind::React => web::feature(dir, &name, feature_kind),
            FrontendKind::Flutter => flutter::feature(dir, &name, feature_kind),
        }),
        Generate::Client {
            package,
            input,
            output,
            name,
            version,
        } => {
            let options = flutter::ClientOptions {
                input,
                output,
                name,
                version,
            };
            frontend_package(package.as_deref(), |kind, dir| match kind {
                FrontendKind::React
                    if options.input.is_some() || options.output.is_some() || options.name.is_some() =>
                {
                    anyhow::bail!(
                        "--input, --output, and --name apply to Flutter packages; React reads orval.config.ts"
                    )
                }
                FrontendKind::React => web::client(dir),
                FrontendKind::Flutter => flutter::client(dir, &options),
            })
        }
        Generate::FlutterApp { name, path } => flutter::app(&name, path.as_deref()),
        backend => dotnet::generate(backend),
    }
}

enum FrontendKind {
    React,
    Flutter,
}

/// Resolves a frontend package directory and tells React from Flutter by its manifest.
fn frontend_package(
    package: Option<&std::path::Path>,
    run: impl FnOnce(FrontendKind, &std::path::Path) -> anyhow::Result<u8>,
) -> anyhow::Result<u8> {
    let dir = match package {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    };
    if dir.join("pubspec.yaml").is_file() {
        run(FrontendKind::Flutter, &dir)
    } else if dir.join("package.json").is_file() {
        run(FrontendKind::React, &dir)
    } else {
        anyhow::bail!("{} has neither pubspec.yaml nor package.json", dir.display())
    }
}
