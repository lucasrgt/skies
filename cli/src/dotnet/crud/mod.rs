//! `skies g crud <Module> <Entity>`: the standard slices for one tenant-scoped `[Entity]`.
//!
//! `List`, `Lookup`, `LookupMy` (only when the entity has a `UserId`), `Create`, `Update`, and `Delete`: plain
//! slices the author owns, the boilerplate the doctor would otherwise make them write six times.
//!
//! The design keeps the output doctor-clean with the entity `g entity` scaffolds. An `[Entity]` has no public
//! setter and no public constructor (SKY0014), so the slices never write columns: Create calls the entity's
//! `Open` factory and Update its `Update` method, both returning `Result<T>` through the entity's private
//! `EnsureValid`. The generator writes those two members into the entity from the fields it reads off the
//! entity's scalar properties (see [`entity`]), keeping any the author already wrote; Delete is a hard remove,
//! a persistence act with no state to guard. Value objects, enums, and other complex properties cannot be built
//! generically, so they are listed in one closing note for the owner to finish.
//!
//! An existing slice is skipped, never clobbered. No tests are emitted: what these slices must guarantee (tenant
//! isolation, not-found on a foreign id) belongs in a spec the author writes, with E2E cases that fail before
//! the change.

mod entity;
mod module;

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::auth::{file_name, missing_package_lines};
use super::error_codes::{self, ErrorCode};
use super::{ApiProject, FRAMEWORK_VERSION, embedded, text};

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

/// Columns the entity, the slice, or the DbContext owns, never the request.
const SYSTEM_FIELDS: &[&str] = &["Id", "OrgId", "CreatedAt", "UpdatedAt", "UserId", "RowVersion"];

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
    scalars: Vec<Field>,
    complex: Vec<Field>,
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
    if !source.contains("ITenantScoped") {
        eprintln!(
            "skies: {entity} is not ITenantScoped — `g crud` currently targets the multi-tenant scaffold (CRUD is \
             tenant-scoped via the module DbContext). Single-tenant CRUD is a later addition."
        );
        return Ok(1);
    }
    if !ENTITY_MARK.is_match(&source) {
        eprintln!(
            "skies: {entity} is not an [Entity] — a persisted type must be one (SKY0021), and `g crud` writes \
             through the entity's own Open/Update. Scaffold it with `skies g entity {module} {entity}`."
        );
        return Ok(1);
    }

    let (scalars, complex) = parse_fields(&source);
    if scalars.is_empty() {
        eprintln!(
            "skies: {entity} has no scalar fields for Create/Update — add its domain state as \
             `public string Name {{ get; private set; }}` properties first."
        );
        return Ok(1);
    }
    let crud = Crud {
        app_name: project.app_name().to_string(),
        app_lower: project.app_lower(),
        module: module.to_string(),
        entity: entity.to_string(),
        plural: detect_db_set(&project.root, entity)?,
        has_user_id: source.contains("public Guid UserId") || source.contains("public Guid? UserId"),
        has_created_at: has_date_property(&source, "CreatedAt"),
        has_updated_at: has_date_property(&source, "UpdatedAt"),
        scalars,
        complex,
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
    if completed != source {
        std::fs::write(&entity_file, completed)?;
        println!("updated {} ({})", entity_file.display(), describe(&added));
    }

    let slices_dir = module_dir.join("Slices");
    let mut emitted = Vec::new();
    for slice in crud.slices() {
        let path = slices_dir.join(format!("{slice}.cs"));
        if path.exists() {
            println!("skipped {} (already present)", path.display());
            continue;
        }
        let template = embedded::dotnet(&format!("crud/{}.cs.cstmpl", slice.replace(entity, "__ENTITY__")));
        text::write(&path, crud.render(template))?;
        println!("created {}", path.display());
        emitted.push(slice);
    }

    // Lookup/Update/Delete return NotFound through a registry constant (SKY0018).
    let not_found = format!("{entity}NotFound");
    let value = format!("{}.not_found", text::hyphenate(entity));
    let summary = format!("No {entity} exists for the given id.");
    let code = ErrorCode {
        name: &not_found,
        value: &value,
        summary: &summary,
    };
    error_codes::ensure(&module_dir, &project.namespace, module, &code)?;

    wire_paging_package(&project.csproj)?;
    module::wire(&module_file, module, &crud.slices())?;
    summarize(&crud, &emitted, &added);
    Ok(0)
}

impl Crud {
    fn slices(&self) -> Vec<String> {
        let e = &self.entity;
        let mut slices = vec![format!("List{e}"), format!("Lookup{e}")];
        if self.has_user_id {
            slices.push(format!("LookupMy{e}"));
        }
        slices.extend([format!("Create{e}"), format!("Update{e}"), format!("Delete{e}")]);
        slices
    }

    fn stamps(&self) -> bool {
        self.has_created_at || self.has_updated_at
    }

    /// The structural tokens first (the longer `__ENTITY_LOWER__` before `__ENTITY__`), then the spliced field
    /// fragments, then the app tokens, exactly as the auth blueprint's token pass.
    fn render(&self, template: &str) -> String {
        let hyphen = text::hyphenate(&self.entity);
        let pick = |on: bool, value: &'static str| if on { value } else { "" };
        let create_params = format!(
            "{}{}",
            pick(self.has_user_id, "ICurrentUser current, "),
            pick(self.stamps(), "TimeProvider clock, ")
        );
        let create_args = format!(
            "{}{}",
            pick(self.has_user_id, "current, "),
            pick(self.stamps(), "clock, ")
        );
        let order = if self.has_created_at {
            "OrderByDescending(e => e.CreatedAt).ThenBy(e => e.Id)"
        } else {
            "OrderBy(e => e.Id)"
        };
        let body = text::fill(
            &text::normalize_newlines(template),
            &[
                ("__MODULE__", &self.module),
                ("__ENTITY_LOWER__", &hyphen),
                ("__ENTITY__", &self.entity),
                ("__PLURAL__", &self.plural),
                ("__INPUT_FIELDS__", &self.input_fields()),
                ("__OPEN_ARGS__", &self.open_args()),
                ("__UPDATE_ARGS__", &self.update_args()),
                (
                    "__AUTH_USING__",
                    pick(self.has_user_id, "using Skies.Framework.Auth;\n\n"),
                ),
                ("__CREATE_PARAMS__", &create_params),
                ("__CREATE_ARGS__", &create_args),
                ("__CLOCK_PARAM__", pick(self.has_updated_at, "TimeProvider clock, ")),
                ("__CLOCK_ARG__", pick(self.has_updated_at, "clock, ")),
                ("__ORDER__", order),
            ],
        );
        text::replace_app_tokens(&body, &self.app_name, &self.app_lower)
    }

    /// `string Name, decimal Price`: the request carries exactly the fields the entity's factory takes.
    fn input_fields(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("{} {}", f.ty, f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn field_args(&self) -> Vec<String> {
        self.scalars.iter().map(|f| format!("input.{}", f.name)).collect()
    }

    /// The positional convention `Open` follows: id, the fields in declaration order, the owner, the clock.
    fn open_args(&self) -> String {
        let mut args = vec!["Guid.NewGuid()".to_string()];
        args.extend(self.field_args());
        if self.has_user_id {
            args.push("current.UserId".to_string());
        }
        if self.stamps() {
            args.push("clock.GetUtcNow().UtcDateTime".to_string());
        }
        args.join(", ")
    }

    fn update_args(&self) -> String {
        let mut args = self.field_args();
        if self.has_updated_at {
            args.push("clock.GetUtcNow().UtcDateTime".to_string());
        }
        args.join(", ")
    }
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

fn has_date_property(source: &str, name: &str) -> bool {
    Regex::new(&format!(r"public\s+DateTime\??\s+{}\s*\{{", regex::escape(name)))
        .expect("date property regex")
        .is_match(source)
}

/// The entity's DbSet name in the shared `AppDb` (`Users` for `User`). Falls back to `<Entity>s` with a note, so
/// the slices still compile once the DbSet is added.
fn detect_db_set(root: &Path, entity: &str) -> Result<String> {
    let fallback = format!("{entity}s");
    let db_file = root.join("AppDb.cs");
    if !db_file.exists() {
        println!("note: no AppDb.cs — assuming DbSet `{fallback}` for {entity}.");
        return Ok(fallback);
    }
    let source = text::read(&db_file)?;
    let pattern = format!(
        r"DbSet<(?:[A-Za-z0-9_]+\.)*{}>\s+(?<plural>[A-Za-z_][A-Za-z0-9_]*)\s*=>",
        regex::escape(entity)
    );
    if let Some(capture) = Regex::new(&pattern).expect("dbset regex").captures(&source) {
        return Ok(capture["plural"].to_string());
    }
    println!(
        "note: no DbSet<{entity}> in AppDb.cs — assuming `{fallback}`. \
         Add `public DbSet<{entity}> {fallback} => Set<{entity}>();` to AppDb.cs."
    );
    Ok(fallback)
}

/// `List` pages through `ToPageAsync` (SKY0027's canonical answer for a set that grows with use), which lives
/// in the `Skies.Framework.EntityFrameworkCore` satellite an app opts into.
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
    if added.row_version {
        parts.push("RowVersion");
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
            "note: {} has complex/VO fields that Open/Update do not set — add them by hand: {}",
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
mod tests {
    use super::*;

    #[test]
    fn splits_scalar_fields_from_complex_and_system_ones() {
        let (scalars, complex) = parse_fields(
            "public Guid Id { get; private set; }\npublic string Name { get; private set; }\n\
             public int? Age { get; set; }\npublic Email Contact { get; private set; }\n\
             public Guid OrgId { get; private set; }\npublic byte[]? RowVersion { get; private set; }\n\
             public string Slug => Name;",
        );
        let names: Vec<_> = scalars.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
        assert_eq!(names, ["string Name", "int? Age"]);
        assert_eq!(complex.len(), 1);
        assert_eq!(complex[0].ty, "Email");
    }

    fn project(entity: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Acme.Api.csproj"), "<Project />").unwrap();
        std::fs::create_dir_all(dir.path().join("Modules/Catalog")).unwrap();
        std::fs::write(dir.path().join("Modules/Catalog/CatalogModule.cs"), "").unwrap();
        std::fs::write(dir.path().join("Modules/Catalog/Product.cs"), entity).unwrap();
        dir
    }

    #[test]
    fn a_data_bag_entity_needs_the_tenant_marker() {
        let dir = project("public class Product { }");
        assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
        assert!(!dir.path().join("Modules/Catalog/Slices").exists());
    }

    #[test]
    fn an_unmarked_tenant_row_is_refused_rather_than_left_failing_sky0021() {
        let dir = project("public class Product : ITenantScoped { public string Name { get; set; } }");
        assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
        assert!(!dir.path().join("Modules/Catalog/Slices").exists());
    }

    #[test]
    fn an_entity_with_no_fields_is_refused() {
        let dir = project("[Entity]\npublic class Product : ITenantScoped { public Guid Id { get; private set; } }");
        assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
    }
}
