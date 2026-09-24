//! `skies g crud <Module> <Entity>`: the standard slices for one `[Entity]`.
//!
//! `List<Entities>`, `Lookup`, `LookupMy` (only when the entity has a `UserId`), `Create`, `Update`, and `Delete`:
//! plain slices the author owns, the boilerplate the doctor would otherwise make them write six times, plus the
//! entity's `<Entity>View` record, the one shape List and Lookup answer with, so the entity itself (its tenancy
//! column) never reaches the wire. Routes follow the collection: `/<module>/<entities>`, `/<entities>/{id}`.
//!
//! The output compiles and is doctor-clean with the entity `g entity` scaffolds. An `[Entity]` has no public
//! setter and no public constructor, so the slices never write columns: Create calls the entity's `Open` factory
//! and Update its `Update` method, both returning `Result<T>` through the entity's private `EnsureValid`. The
//! generator writes those two members into the entity from the fields it reads off the entity's scalar
//! properties (see [`entity`]), keeping any the author already wrote; Delete is a hard remove, a persistence act
//! with no state to guard. Value objects, enums, and other complex properties cannot be built generically, so
//! they are listed in one closing note for the owner to finish.
//!
//! Update and Delete are optimistic: the view carries the entity's `Version` token, the client sends it back, and a
//! save against a row that changed in between answers `409` (`<module>.<entity>_changed`) instead of overwriting it.
//!
//! The entity's `DbSet` is registered in the app's `AppDb` (see [`super::app_db`]). An `ITenantScoped` entity is
//! scoped by the DbContext's tenant filter, and all six slices map under the module's route group, inheriting its
//! authorization decision exactly as `g slice` does. Any other entity is app-wide, shared by every org, so a
//! signed-in user may read it but its writes map under the module's admin group (see [`module::wire_admin`]):
//! `AppPolicies.AppAdmin` where the auth blueprint defines it, and closed to everyone where it does not.
//!
//! An existing slice is skipped, never clobbered. No tests are emitted: what these slices must guarantee (tenant
//! isolation, not-found on a foreign id) belongs in a spec the author writes, with E2E cases that fail before
//! the change.

mod entity;
pub(super) mod module;
mod render;

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::app_db::{self, Registration};
use super::auth::{file_name, missing_package_lines};
use super::error_codes::{self, ErrorCode};
use super::{ApiProject, FRAMEWORK_VERSION, embedded, text};
use render::ViewField;

/// Types taken straight from a request into the entity's factory. Anything else is "complex".
const SCALAR_TYPES: &[&str] = &[
    "string",
    "string?",
    "bool",
    "bool?",
    "int",
    "int?",
    "long",
    "long?",
    "double",
    "double?",
    "decimal",
    "decimal?",
    "DateTime",
    "DateTime?",
    "Guid",
    "Guid?",
];

/// Columns the entity, the slice, or the DbContext owns, never the request's fields.
const SYSTEM_FIELDS: &[&str] = &[
    "Id",
    "OrgId",
    "TenantId",
    "CreatedAt",
    "UpdatedAt",
    "UserId",
    "RowVersion",
    "Version",
];

/// Columns that are persistence, not contract: never on the entity's view record.
const HIDDEN_FIELDS: &[&str] = &["OrgId", "TenantId", "RowVersion"];

/// A property with a setter, public or private: the entity's state.
static PROPERTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"public\s+(?<type>[A-Za-z0-9_<>,\.\?\[\] ]+?)\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)",
        r"\s*\{\s*get;\s*(?:private\s+)?set;\s*\}",
    ))
    .expect("property regex")
});

static ENTITY_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(?:[A-Za-z.]*\.)?Entity(?:Attribute)?\]").expect("entity mark regex"));

struct Field {
    name: String,
    ty: String,
}

/// Everything a template needs about the target entity.
struct Crud {
    app_name: String,
    app_lower: String,
    module: String,
    entity: String,
    plural: String,
    has_user_id: bool,
    has_created_at: bool,
    has_updated_at: bool,
    tenant_scoped: bool,
    /// The module's route group decides authorization, so the slices state no posture of their own.
    group_decides: bool,
    /// The app defines `AppPolicies.AppAdmin` (the auth blueprint), the policy app-wide writes require.
    app_admin: bool,
    scalars: Vec<Field>,
    complex: Vec<Field>,
    view: Vec<ViewField>,
}

pub fn generate(root: &Path, module: &str, entity: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let module_dir = project.module_dir(module);
    let module_file = module_dir.join(format!("{module}Module.cs"));
    if !module_file.exists() {
        eprintln!(
            "skies: no {module} module here — run `skies g module {module}` (or `skies g auth` for Account) first."
        );
        return Ok(1);
    }
    let entity_file = module_dir.join(format!("{entity}.cs"));
    if !entity_file.exists() {
        eprintln!("skies: no {entity} entity in {module} — run `skies g entity {module} {entity}` first.");
        return Ok(1);
    }
    let source = text::read(&entity_file)?;
    if !ENTITY_MARK.is_match(&source) {
        eprintln!(
            "skies: {entity} is not an [Entity]. A persisted type must be one, and `g crud` writes through the \
             entity's own Open/Update. Scaffold it with `skies g entity {module} {entity}`."
        );
        return Ok(1);
    }
    let Some(app_db) = app_db::locate(&project.root)? else {
        eprintln!(
            "skies: no AppDb in {}. The crud slices query the app's DbContext as `AppDb`: run `skies g auth` \
             (it adds AppDb.cs) or declare `public class AppDb : DbContext` first.",
            project.root.display()
        );
        return Ok(1);
    };
    let (scalars, complex) = parse_fields(&source);
    if scalars.is_empty() {
        eprintln!(
            "skies: {entity} has no scalar fields for Create/Update — add its domain state as \
             `public string Name {{ get; private set; }}` properties first."
        );
        return Ok(1);
    }

    let plural = match app_db::register(&app_db, &project.namespace, module, entity)? {
        Registration::Present(name) => name,
        Registration::Added(name) => {
            println!("registered DbSet<{entity}> {name} in {}", app_db.display());
            name
        }
        Registration::Manual(lines) => {
            for line in &lines {
                println!("note: add `{line}` to {}", app_db.display());
            }
            text::plural(entity)
        }
    };
    let mut crud = Crud {
        app_name: project.app_name().to_string(),
        app_lower: project.app_lower(),
        module: module.to_string(),
        entity: entity.to_string(),
        plural,
        has_user_id: source.contains("public Guid UserId") || source.contains("public Guid? UserId"),
        has_created_at: has_date_property(&source, "CreatedAt"),
        has_updated_at: has_date_property(&source, "UpdatedAt"),
        tenant_scoped: is_tenant_scoped(&source, entity),
        group_decides: module::group_decides(&text::read(&module_file)?),
        app_admin: defines_app_admin(&project.root)?,
        scalars,
        complex,
        view: view_fields(&source),
    };

    let shape = entity::Shape {
        entity,
        fields: &crud.scalars,
        has_user_id: crud.has_user_id,
        has_created_at: crud.has_created_at,
        has_updated_at: crud.has_updated_at,
    };
    let (completed, added) = match entity::complete(&source, &shape) {
        Ok(done) => done,
        Err(reason) => {
            eprintln!("skies: {reason} — `g crud` needs the shape `skies g entity` scaffolds.");
            return Ok(1);
        }
    };
    // The view reads the completed entity, so it carries the Version token complete may just have added.
    crud.view = view_fields(&completed);
    if completed != source {
        std::fs::write(&entity_file, completed)?;
        println!("updated {} ({})", entity_file.display(), describe(&added));
    }

    let view = module_dir.join(format!("{entity}View.cs"));
    if view.exists() {
        println!("skipped {} (already present)", view.display());
    } else {
        text::write(
            &view,
            crud.render(embedded::dotnet("crud/__ENTITY__View.cs.cstmpl"), false),
        )?;
        println!("created {}", view.display());
    }

    let slices_dir = module_dir.join("Slices");
    let slices = crud.slices();
    let mut emitted = Vec::new();
    for slice in &slices {
        let path = slices_dir.join(format!("{}.cs", slice.name));
        if path.exists() {
            println!("skipped {} (already present)", path.display());
            continue;
        }
        text::write(&path, crud.render(embedded::dotnet(slice.template), slice.writes))?;
        println!("created {}", path.display());
        emitted.push(slice.name.clone());
    }

    let snake = text::hyphenate(entity).replace('-', "_");
    let prefix = module.to_lowercase();
    for (name, value, summary) in [
        (
            format!("{entity}NotFound"),
            format!("{prefix}.{snake}_not_found"),
            format!("No {entity} exists for the given id."),
        ),
        (
            format!("{entity}Changed"),
            format!("{prefix}.{snake}_changed"),
            format!("The {entity} changed since the client read it; reload it and apply the change again."),
        ),
    ] {
        let code = ErrorCode {
            name: &name,
            value: &value,
            summary: &summary,
        };
        error_codes::ensure(&module_dir, &project.namespace, module, &code)?;
    }

    wire_paging_package(&project.csproj)?;
    let app_wide_writes = |slice: &&render::Slice| slice.writes && !crud.tenant_scoped;
    let grouped: Vec<String> = slices
        .iter()
        .filter(|s| !app_wide_writes(s))
        .map(|s| s.name.clone())
        .collect();
    let admin: Vec<String> = slices.iter().filter(app_wide_writes).map(|s| s.name.clone()).collect();
    module::wire(&module_file, module, &grouped)?;
    if !admin.is_empty() {
        module::wire_admin(&module_file, module, &admin, crud.app_admin)?;
    }
    summarize(&crud, &emitted, &added);
    Ok(0)
}

/// Whether the auth blueprint's `AppPolicies.AppAdmin` exists in the API project, for app-wide writes to require.
fn defines_app_admin(root: &Path) -> Result<bool> {
    let policies = root.join("AppPolicies.cs");
    Ok(policies.is_file() && text::read(&policies)?.contains("const string AppAdmin"))
}

/// Reads the entity's settable properties, splitting scalars from complex fields and dropping system columns.
fn parse_fields(source: &str) -> (Vec<Field>, Vec<Field>) {
    let mut scalars = Vec::new();
    let mut complex = Vec::new();
    for capture in PROPERTY.captures_iter(source) {
        let field = Field {
            name: capture["name"].trim().to_string(),
            ty: capture["type"].trim().to_string(),
        };
        if SYSTEM_FIELDS.contains(&field.name.as_str()) {
            continue;
        }
        if SCALAR_TYPES.contains(&field.ty.as_str()) {
            scalars.push(field);
        } else {
            complex.push(field);
        }
    }
    (scalars, complex)
}

/// Every scalar property, system columns included (`Id`, `UserId`, the stamps), minus the persistence ones: the
/// entity as a client may see it.
fn view_fields(source: &str) -> Vec<ViewField> {
    PROPERTY
        .captures_iter(source)
        .map(|capture| ViewField {
            name: capture["name"].trim().to_string(),
            ty: capture["type"].trim().to_string(),
        })
        .filter(|field| SCALAR_TYPES.contains(&field.ty.as_str()) && !HIDDEN_FIELDS.contains(&field.name.as_str()))
        .collect()
}

/// Whether the entity's own declaration carries the tenant marker, so the DbContext's filter scopes it.
fn is_tenant_scoped(source: &str, entity: &str) -> bool {
    Regex::new(&format!(r"class\s+{}\b[^{{]*\bITenantScoped\b", regex::escape(entity)))
        .expect("tenant marker regex")
        .is_match(source)
}

fn has_date_property(source: &str, name: &str) -> bool {
    Regex::new(&format!(r"public\s+DateTime\??\s+{}\s*\{{", regex::escape(name)))
        .expect("date property regex")
        .is_match(source)
}

/// `List` pages through `ToPageAsync` (the canonical answer for a set that grows with use), which lives in the
/// `Skies.Framework.EntityFrameworkCore` satellite an app opts into.
fn wire_paging_package(csproj: &Path) -> Result<()> {
    let current = text::read(csproj)?;
    let missing = missing_package_lines(&current, &[("Skies.Framework.EntityFrameworkCore", FRAMEWORK_VERSION)]);
    if missing.is_empty() {
        return Ok(());
    }
    let nl = text::newline_of(&current);
    std::fs::write(
        csproj,
        text::insert_before_closing_item_group(&current, &missing.join(nl), nl),
    )?;
    println!("added Skies.Framework.EntityFrameworkCore to {}", file_name(csproj));
    Ok(())
}

fn describe(added: &entity::Added) -> String {
    let mut parts = Vec::new();
    if added.open {
        parts.push("Open");
    }
    if added.update {
        parts.push("Update");
    }
    if added.version {
        parts.push("Version");
    }
    format!("added {}", parts.join(", "))
}

fn summarize(crud: &Crud, emitted: &[String], added: &entity::Added) {
    if !crud.has_user_id {
        println!(
            "note: {0} has no UserId — LookupMy{0} was not generated (the \"me\" lookup needs an owner column).",
            crud.entity
        );
    }
    if added.kept_update {
        println!(
            "note: kept {0}.Update as written; it must renew Version (`Version = Guid.NewGuid();`) or Update{0} cannot \
             tell a stale write from a fresh one.",
            crud.entity
        );
    }
    if !crud.tenant_scoped && !crud.app_admin {
        println!(
            "note: {0} is app-wide and this app has no AppPolicies.AppAdmin (the auth blueprint's), so its writes are \
             mapped closed to everyone; name the policy that may change {0} in {1}Module.Map.",
            crud.entity, crud.module
        );
    }
    if added.kept_open {
        println!(
            "note: kept {0}.Open as written; Create{0} calls it as Open({1}).",
            crud.entity,
            crud.open_args()
        );
    }
    if !crud.complex.is_empty() {
        let list: Vec<String> = crud.complex.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
        println!(
            "note: {} has complex/VO fields that Open/Update and {}View do not carry — add them by hand: {}",
            crud.entity,
            crud.entity,
            list.join(", ")
        );
    }
    let what = if emitted.is_empty() {
        "nothing new (all slices already present)".to_string()
    } else {
        emitted.join(", ")
    };
    println!(
        "crud generated for {}/{} — {what}. Add {}'s invariants to EnsureValid, write down how it can fail in a \
         spec (`skies spec new`), then prove it.",
        crud.module, crud.entity, crud.entity
    );
}

#[cfg(test)]
mod tests;
