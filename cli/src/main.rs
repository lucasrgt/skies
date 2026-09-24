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
    /// Run the architecture doctors: dotnet build (SKY*), eslint (SKYFE*), and the Flutter rules (SKYFL*).
    Doctor {
        /// Check only this package directory (a Flutter or React package, or a .NET project or its folder), so a
        /// package's own `lint` script can call the doctor.
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

#[derive(Subcommand)]
pub enum Generate {
    /// A module: <Name>Module.cs, error codes, ctx.md, wired into the module registry.
    Module { name: String },
    /// A slice inside a module.
    Slice { module: String, name: String },
    /// A rich [Entity] with an EnsureValid invariant funnel.
    Entity { module: String, name: String },
    /// An always-valid [ValueObject] in BuildingBlocks.
    Vo { name: String },
    /// List/lookup/create/update/delete slices for a tenant-scoped entity.
    Crud { module: String, entity: String },
    /// A SignalR hub for real-time fan-out.
    Hub { module: String, name: String },
    /// The auth module: register, login, refresh, logout, me, sessions.
    Auth {
        #[arg(long)]
        skip_tenancy: bool,
        #[arg(long)]
        skip_cookies: bool,
    },
    /// Phone verification by SMS code.
    #[command(name = "auth:otp")]
    AuthOtp,
    /// Google sign-up and sign-in.
    #[command(name = "auth:oauth")]
    AuthOauth,
    /// Email verification and password reset.
    #[command(name = "auth:email")]
    AuthEmail,
    /// A frontend feature (ViewModel + View + i18n) in a React or Flutter package.
    Feature {
        name: String,
        /// The frontend package directory (defaults to the current directory).
        #[arg(long)]
        package: Option<PathBuf>,
    },
    /// The typed API client for a React or Flutter package, from the backend's OpenAPI contract.
    Client {
        /// The frontend package directory (defaults to the current directory).
        #[arg(long)]
        package: Option<PathBuf>,
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
    Record {
        /// The spec id or folder name.
        spec: String,
        /// The revision the failure modes must fail on. Defaults to the merge-base with the default branch.
        #[arg(long)]
        red: Option<String>,
        /// A patch applied to the red checkout before running, for specs written after the code.
        #[arg(long)]
        red_patch: Option<PathBuf>,
    },
    /// List receipts that are current and receipts whose inputs changed. Hashes only; runs nothing.
    Status,
    /// Rerun specs and refresh their green evidence.
    Verify {
        /// Spec ids or folder names.
        specs: Vec<String>,
        /// Rerun every spec whose receipt is stale.
        #[arg(long)]
        stale: bool,
        /// Rerun every spec.
        #[arg(long)]
        all: bool,
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
        Command::Proof(Proof::Record { spec, red, red_patch }) => {
            proof::record(&spec, red.as_deref(), red_patch.as_deref())
        }
        Command::Proof(Proof::Status) => proof::status(),
        Command::Proof(Proof::Verify { specs, stale, all }) => proof::verify(&specs, stale, all),
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
        Generate::Feature { name, package } => frontend_package(package.as_deref(), |kind, dir| match kind {
            FrontendKind::React => web::feature(dir, &name),
            FrontendKind::Flutter => flutter::feature(dir, &name),
        }),
        Generate::Client { package } => frontend_package(package.as_deref(), |kind, dir| match kind {
            FrontendKind::React => web::client(dir),
            FrontendKind::Flutter => flutter::client(dir),
        }),
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
