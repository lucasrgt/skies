//! `skies g auth:otp|auth:oauth|auth:email`: augment a generated Account module with a provider-backed flow.
//!
//! Emits the flow's slices and entities, then edits the existing `User`, `AppDb`, `AccountModule`,
//! `AccountSetup`, and the API csproj to wire them in. Every edit is idempotent (re-running never duplicates a
//! field, DbSet, map line, registration, or reference) and a missing anchor prints the manual step instead of
//! failing. The flow's tests arrive as a spec, `.specs/<id>-auth-<flow>/`.
//!
//! Only the default multi-tenant scaffold is supported; a `--skip-tenancy` app is rejected with a clear message.

use std::path::Path;

use anyhow::Result;

use super::auth::{file_name, missing_package_lines};
use super::blueprint::{self, Flags};
use super::error_codes::{self, ErrorCode};
use super::flow_specs::{Flow, FlowSpec};
use super::{ApiProject, FRAMEWORK_VERSION, embedded, specs, text};

pub fn generate(root: &Path, flow: Flow) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let spec = flow.spec();
    let account = project.module_dir("Account");
    let account_module = account.join("AccountModule.cs");
    if !account_module.exists() {
        eprintln!("skies: no Account module here — run `skies g auth` first.");
        return Ok(1);
    }
    let user_file = account.join("User.cs");
    if user_file.exists() && !text::read(&user_file)?.contains("ITenantScoped") {
        eprintln!(
            "skies: auth:{} currently supports the default (multi-tenant) scaffold; this app was generated with \
             --skip-tenancy.",
            spec.token
        );
        return Ok(1);
    }

    emit_templates(&project, spec)?;
    for &(name, value, summary) in spec.error_codes {
        error_codes::ensure(
            &account,
            &project.namespace,
            "Account",
            &ErrorCode { name, value, summary },
        )?;
    }
    augment_user(&user_file, spec)?;
    augment_app_db(&project.root.join("AppDb.cs"), spec)?;
    augment_account_module(&account_module, spec)?;
    augment_account_setup(&account.join("AccountSetup.cs"), spec)?;
    augment_api_project(&project.csproj, spec)?;
    let folder = specs::emit(&project, spec.folder, Flags::DEFAULT)?.folder;

    println!(
        "{} Its failure modes and E2E are in {}.",
        spec.summary,
        folder.display()
    );
    Ok(0)
}

/// Emits the flow's files, then its shared folders' files, into the API project. A file that already exists (the
/// shared verification entity and store when a second flow needs them) is skipped, never clobbered.
fn emit_templates(project: &ApiProject, spec: &FlowSpec) -> Result<()> {
    let (app_name, app_lower) = (project.app_name(), project.app_lower());
    let folders = std::iter::once(spec.folder).chain(spec.shared_folders.iter().copied());
    for (logical, body) in folders.flat_map(embedded::dotnet_folder) {
        let destination = project
            .root
            .join(blueprint::render_path(&logical, app_name, &app_lower));
        if destination.exists() {
            println!("skipped {} (already present)", destination.display());
            continue;
        }
        text::write(
            &destination,
            blueprint::render(body, app_name, &app_lower, Flags::DEFAULT),
        )?;
        println!("created {}", destination.display());
    }
    Ok(())
}

/// `User` is an encapsulated `[Entity]` (SKY0014/SKY0021), so a flow grows it through intention-revealing
/// members, never public setters. Fields sit after `CreatedAt`; methods before the `EnsureValid` funnel.
fn augment_user(user_file: &Path, spec: &FlowSpec) -> Result<()> {
    if spec.user_fields.is_empty() && spec.user_methods.is_empty() {
        return Ok(());
    }
    if !user_file.exists() {
        for field in spec.user_fields {
            println!("note: add `{}` to Modules/Account/User.cs", field.trim());
        }
        for method in spec.user_methods {
            println!("note: add this member to Modules/Account/User.cs:\n{}", method.code);
        }
        return Ok(());
    }

    let mut source = text::read(user_file)?;
    let nl = text::newline_of(&source);
    let mut changed = false;

    let fields: Vec<&str> = spec
        .user_fields
        .iter()
        .copied()
        .filter(|f| !contains_member(&source, f))
        .collect();
    if !fields.is_empty() {
        const ANCHOR: &str = "    public DateTime CreatedAt { get; private set; }";
        if source.contains(ANCHOR) {
            let block = fields
                .iter()
                .map(|f| f.replace('\n', nl))
                .collect::<Vec<_>>()
                .join(&format!("{nl}{nl}"));
            source = text::replace_first(&source, ANCHOR, &format!("{ANCHOR}{nl}{nl}{block}"));
            changed = true;
        } else {
            for field in &fields {
                println!("note: add `{}` to {}", field.trim(), user_file.display());
            }
        }
    }

    let methods: Vec<_> = spec.user_methods.iter().filter(|m| !source.contains(m.token)).collect();
    if !methods.is_empty() {
        const ANCHOR: &str = "    private Result<User> EnsureValid()";
        if source.contains(ANCHOR) {
            let block = methods
                .iter()
                .map(|m| m.code.replace('\n', nl))
                .collect::<Vec<_>>()
                .join(&format!("{nl}{nl}"));
            source = text::replace_first(&source, ANCHOR, &format!("{block}{nl}{nl}{ANCHOR}"));
            changed = true;
        } else {
            for method in &methods {
                println!("note: add this member to {}:\n{}", user_file.display(), method.code);
            }
        }
    }

    if changed {
        std::fs::write(user_file, source)?;
        println!(
            "augmented User.cs ({} field(s), {} method(s))",
            fields.len(),
            methods.len()
        );
    }
    Ok(())
}

/// A field is present when its property name already appears as `Name {`, so a differing type or default on
/// the owner's side never causes a duplicate.
fn contains_member(source: &str, field: &str) -> bool {
    let Some(brace) = field.find('{') else { return false };
    let name = field[..brace].trim_end().rsplit(' ').next().unwrap_or_default();
    !name.is_empty() && source.contains(&format!("{name} {{"))
}

/// DbSets go after the `UserSessions` DbSet; indexes after the session `FamilyId` index.
fn augment_app_db(db_file: &Path, spec: &FlowSpec) -> Result<()> {
    if !db_file.exists() {
        println!("note: no AppDb.cs — add the flow's DbSet(s) and index(es) by hand.");
        return Ok(());
    }
    let mut source = text::read(db_file)?;
    let nl = text::newline_of(&source);
    let mut changed = false;

    for &(entity, declaration) in spec.db_sets {
        if source.contains(&format!("DbSet<{entity}>")) {
            continue;
        }
        const ANCHOR: &str = "    public DbSet<UserSession> UserSessions => Set<UserSession>();";
        if source.contains(ANCHOR) {
            source = text::replace_first(&source, ANCHOR, &format!("{ANCHOR}{nl}{nl}{declaration}"));
            changed = true;
        } else {
            println!("note: add `{}` to AppDb.cs", declaration.trim());
        }
    }
    for &index in spec.indexes {
        if source.contains(index.trim()) {
            continue;
        }
        const ANCHOR: &str = "        session.HasIndex(s => s.FamilyId);";
        if source.contains(ANCHOR) {
            source = text::replace_first(&source, ANCHOR, &format!("{ANCHOR}{nl}{nl}{index}"));
            changed = true;
        } else {
            println!("note: add `{}` to AppDb.cs OnModelCreating", index.trim());
        }
    }

    if changed {
        std::fs::write(db_file, source)?;
        println!("wired DbSet(s) + index(es) into AppDb.cs");
    }
    Ok(())
}

/// The routes join the `/account` group before the closing brace of `Map`.
fn augment_account_module(module_file: &Path, spec: &FlowSpec) -> Result<()> {
    let source = text::read(module_file)?;
    let nl = text::newline_of(&source);
    let missing: Vec<&str> = spec
        .map_lines
        .iter()
        .copied()
        .filter(|m| !source.contains(m.trim()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let anchor = format!("{nl}    }}{nl}}}");
    if source.contains(&anchor) {
        let block = format!("{nl}{}{anchor}", missing.join(nl));
        std::fs::write(module_file, text::replace_first(&source, &anchor, &block))?;
        println!("wired {} slice map(s) into AccountModule.cs", missing.len());
    } else {
        for line in missing {
            println!("note: add `{}` to AccountModule.Map", line.trim());
        }
    }
    Ok(())
}

/// The flow's registrations join the module's composition after `AddAuthorization()`, with the provider's using
/// grouped after `Skies.Framework.Auth`. A line already present (the verification service a second flow shares) is
/// kept as is.
fn augment_account_setup(setup_file: &Path, spec: &FlowSpec) -> Result<()> {
    if !setup_file.exists() {
        for line in spec.di_lines {
            println!("note: no AccountSetup.cs — register `{line}` in AddAccount.");
        }
        return Ok(());
    }
    let mut source = text::read(setup_file)?;
    let nl = text::newline_of(&source);
    let mut changed = false;

    let using = format!("using {};", spec.provider_namespace);
    if !source.contains(&using) {
        source = text::replace_first(
            &source,
            "using Skies.Framework.Auth;",
            &format!("using Skies.Framework.Auth;{nl}{using}"),
        );
        changed = true;
    }
    let mut after = "        builder.Services.AddAuthorization();".to_string();
    let missing: Vec<&str> = spec
        .di_lines
        .iter()
        .copied()
        .filter(|line| !source.contains(line))
        .collect();
    for line in missing {
        if source.contains(&after) {
            let inserted = format!("{after}{nl}        {line}");
            source = text::replace_first(&source, &after, &inserted);
            after = format!("        {line}");
            changed = true;
        } else {
            println!("note: add `{line}` to AddAccount in AccountSetup.cs");
        }
    }

    if changed {
        std::fs::write(setup_file, source)?;
        println!("registered {} provider in AccountSetup.cs", spec.provider_namespace);
    }
    Ok(())
}

/// Matches the csproj's existing framework reference style: a ProjectReference beside `Skies.Framework.AspNetCore`
/// when the app is co-developed against a framework checkout, a PackageReference otherwise.
fn augment_api_project(csproj: &Path, spec: &FlowSpec) -> Result<()> {
    let source = text::read(csproj)?;
    if source.contains(spec.package_id) {
        return Ok(());
    }
    let nl = text::newline_of(&source);
    let reference = match sibling_project_reference(&source, spec.package_id) {
        Some(path) => format!("    <ProjectReference Include=\"{path}\" />"),
        None => missing_package_lines(&source, &[(spec.package_id, FRAMEWORK_VERSION)]).join(nl),
    };
    std::fs::write(csproj, text::insert_before_closing_item_group(&source, &reference, nl))?;
    println!("added {} reference to {}", spec.package_id, file_name(csproj));
    Ok(())
}

/// For `<ProjectReference Include="../fw/src/Skies.Framework.AspNetCore/Skies.Framework.AspNetCore.csproj" />`,
/// the path of the sibling package project, keeping the reference's own separator style.
fn sibling_project_reference(csproj: &str, package_id: &str) -> Option<String> {
    let include = csproj.lines().map(str::trim).find_map(|line| {
        if !line.starts_with("<ProjectReference") || !line.contains("Skies.Framework.AspNetCore.csproj") {
            return None;
        }
        let start = line.find("Include=\"")? + "Include=\"".len();
        let end = line[start..].find('"')? + start;
        Some(line[start..end].to_string())
    })?;
    let separator = if include.contains('\\') { '\\' } else { '/' };
    let project_dir = &include[..include.rfind(['/', '\\'])?];
    let packages_dir = &project_dir[..project_dir.rfind(['/', '\\']).map_or(0, |i| i + 1)];
    Some(format!("{packages_dir}{package_id}{separator}{package_id}.csproj"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_co_developed_app_gets_a_sibling_project_reference() {
        let csproj = "  <ItemGroup>\n    <ProjectReference Include=\"..\\..\\fw\\src\\Skies.Framework.AspNetCore\\Skies.Framework.AspNetCore.csproj\" />\n";
        assert_eq!(
            sibling_project_reference(csproj, "Skies.Framework.Sms").unwrap(),
            "..\\..\\fw\\src\\Skies.Framework.Sms\\Skies.Framework.Sms.csproj"
        );
        let unix =
            "<ProjectReference Include=\"/fw/src/Skies.Framework.AspNetCore/Skies.Framework.AspNetCore.csproj\" />";
        assert_eq!(
            sibling_project_reference(unix, "Skies.Framework.Mail").unwrap(),
            "/fw/src/Skies.Framework.Mail/Skies.Framework.Mail.csproj"
        );
        assert!(
            sibling_project_reference("<PackageReference Include=\"Skies.Framework.AspNetCore\" />", "X").is_none()
        );
    }

    #[test]
    fn members_are_detected_by_name_whatever_their_type() {
        assert!(contains_member(
            "    public bool? IsEmailVerified { get; set; }",
            "    public bool IsEmailVerified { get; private set; }"
        ));
        assert!(!contains_member(
            "    public bool Other { get; }",
            "    public bool IsEmailVerified { get; private set; }"
        ));
    }
}
