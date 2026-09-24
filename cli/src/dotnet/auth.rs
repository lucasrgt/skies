//! `skies g auth [--skip-tenancy] [--skip-cookies]`: the Account module from the proven blueprint.
//!
//! Register, login, refresh, logout, me, and session management, plus, unless opted out, multi-tenant scoping and
//! web-cookie refresh delivery. The auth *mechanism* is not emitted: reading the caller, minting and validating JWTs,
//! password hashing, refresh rotation with the family burn on replay, revocation, and cookie delivery are the
//! `Skies.Framework.Auth` package, wired by one `AddSkiesAuth` call in `AccountSetup`, so a security fix reaches the
//! app through a package version. What is emitted is plain C# the app owns: the slices (input, output, error codes,
//! auth posture), the entities, and the small store that keeps the session table.
//!
//! The blueprint's tests arrive as a spec, `.specs/<id>-auth/`: the failure modes an auth module can have and
//! the E2E cases that prove this one does not, so the app starts with a receipt-ready proof of its riskiest code.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::blueprint::{self, Flags};
use super::{ApiProject, FRAMEWORK_VERSION, embedded, first_csproj, specs, text};

pub fn generate(root: &Path, tenancy: bool, cookies: bool) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    if project.module_dir("Account").join("AccountModule.cs").exists() {
        eprintln!("skies: an Account module already exists here — remove it first.");
        return Ok(1);
    }

    let flags = Flags { tenancy, cookies };
    let (app_name, app_lower) = (project.app_name().to_string(), project.app_lower());
    for (logical, body) in embedded::dotnet_folder("auth") {
        if !tenancy && logical.starts_with("Tenancy/") {
            continue;
        }
        let destination = destination(&project, &blueprint::render_path(&logical, &app_name, &app_lower));
        text::write(&destination, blueprint::render(body, &app_name, &app_lower, flags))?;
        println!("created {}", destination.display());
    }

    wire_program(&project)?;
    wire_api_project(&project.csproj)?;
    wire_test_project(&project.test_dir())?;
    wire_global_usings(&project)?;
    let spec = specs::emit(&project, "auth", flags)?;
    // The ctx cites the spec that proves its invariants; the spec's id and numbering exist only now.
    spec.cite_file(&project.module_dir("Account").join("Account.ctx.md"))?;

    println!("{}", summary(flags, &spec.folder));
    Ok(0)
}

/// Blueprint files under `Tests/` belong to the tests project (`tests/<App>.Tests`); the rest land in the API
/// project at their logical path.
fn destination(project: &ApiProject, rendered: &str) -> PathBuf {
    match rendered.strip_prefix("Tests/") {
        Some(rest) => project.test_dir().join(rest),
        None => project.root.join(rendered),
    }
}

/// Two lines in `Program.cs`: `builder.AddAccount();` before `Build()` and `AccountModule.Map(app);` before
/// `app.Run();`. No middleware is wired: `WebApplication` adds authentication and authorization itself once
/// `AddAccount` registered them, so the composition root stays the thin index SKY0017 requires.
fn wire_program(project: &ApiProject) -> Result<()> {
    let program = project.root.join("Program.cs");
    if !program.exists() {
        println!("note: no Program.cs — register auth with builder.AddAccount(); and AccountModule.Map(app);");
        return Ok(());
    }

    let mut source = text::read(&program)?;
    if source.contains("AccountModule.Map") {
        return Ok(());
    }
    let nl = text::newline_of(&source);
    let using = format!("using {}.Api.Modules.Account;{nl}", project.app_name());
    if !source.contains(&using) {
        source = format!("{using}{source}");
    }

    if !source.contains("var builder") || !source.contains("var app =") || !source.contains("app.Run();") {
        std::fs::write(&program, source)?;
        println!(
            "note: Program.cs looks unusual — add builder.AddAccount(); before Build(), then \
             AccountModule.Map(app); before app.Run(); (WebApplication auto-adds the auth middleware)."
        );
        return Ok(());
    }

    source = text::replace_first(&source, "var app =", &format!("builder.AddAccount();{nl}{nl}var app ="));
    source = text::replace_first(
        &source,
        "app.Run();",
        &format!("AccountModule.Map(app);{nl}{nl}app.Run();"),
    );
    std::fs::write(&program, source)?;
    println!("wired auth into Program.cs (builder.AddAccount(); + AccountModule.Map(app);)");
    Ok(())
}

/// The data packages the Account module needs, plus `Skies.Framework.Auth`, which carries the auth mechanism (and
/// JwtBearer and argon2id transitively) so the app names no JWT or crypto package itself.
fn wire_api_project(csproj: &Path) -> Result<()> {
    let current = text::read(csproj)?;
    let packages = [
        ("Microsoft.EntityFrameworkCore", "10.0.8"),
        ("Microsoft.EntityFrameworkCore.InMemory", "10.0.8"),
        ("Skies.Framework.Auth", FRAMEWORK_VERSION),
    ];
    let missing = missing_package_lines(&current, &packages);
    if missing.is_empty() {
        return Ok(());
    }
    let nl = text::newline_of(&current);
    std::fs::write(
        csproj,
        text::insert_before_closing_item_group(&current, &missing.join(nl), nl),
    )?;
    println!("added auth package references to {}", file_name(csproj));
    Ok(())
}

/// The tests boot the real app over an isolated in-memory store per test (`TestApp`), so the tests project needs
/// the optional `Skies.Framework.Testing.InMemory` package beside `Skies.Framework.Testing`.
fn wire_test_project(test_dir: &Path) -> Result<()> {
    let Some(csproj) = first_csproj(test_dir) else {
        println!(
            "note: no test project at {} — add a <App>.Tests project with the \
             Skies.Framework.Testing.InMemory package so the spec E2E can boot the app.",
            test_dir.display()
        );
        return Ok(());
    };
    let current = text::read(&csproj)?;
    let missing = missing_package_lines(&current, &[("Skies.Framework.Testing.InMemory", FRAMEWORK_VERSION)]);
    if missing.is_empty() {
        return Ok(());
    }
    let nl = text::newline_of(&current);
    std::fs::write(
        &csproj,
        text::insert_before_closing_item_group(&current, &missing.join(nl), nl),
    )?;
    println!("added auth test package references to {}", file_name(&csproj));
    Ok(())
}

/// The Account module's `Email` value object lives in `BuildingBlocks`, which every slice uses.
fn wire_global_usings(project: &ApiProject) -> Result<()> {
    let path = project.root.join("GlobalUsings.cs");
    let line = format!("global using {}.Api.BuildingBlocks;", project.app_name());
    if !path.exists() {
        text::write(&path, format!("{line}\n"))?;
        println!("created {}", path.display());
        return Ok(());
    }
    let current = text::read(&path)?;
    if current.contains(&line) {
        return Ok(());
    }
    let nl = text::newline_of(&current);
    std::fs::write(&path, format!("{}{nl}{line}{nl}", current.trim_end()))?;
    println!("added BuildingBlocks global using");
    Ok(())
}

/// The `<PackageReference>` lines for the packages the csproj does not reference yet, 4-space indented.
pub(super) fn missing_package_lines(csproj: &str, packages: &[(&str, &str)]) -> Vec<String> {
    packages
        .iter()
        .filter(|(name, _)| !csproj.contains(&format!("PackageReference Include=\"{name}\"")))
        .map(|(name, version)| format!("    <PackageReference Include=\"{name}\" Version=\"{version}\" />"))
        .collect()
}

pub(super) fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

fn summary(flags: Flags, spec: &Path) -> String {
    format!(
        "auth generated — {}, {}. Its failure modes and E2E are in {}; run them with `dotnet test`.",
        if flags.tenancy { "multi-tenant" } else { "single-tenant" },
        if flags.cookies {
            "web-cookie + body delivery"
        } else {
            "body-only delivery"
        },
        spec.display()
    )
}
