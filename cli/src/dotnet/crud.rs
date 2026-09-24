//! `skies g crud <Module> <Entity>`: the standard slices for one tenant-scoped entity.
//!
//! `List`, `Lookup`, `LookupMy` (only when the entity has a `UserId`), `Create`, `Update`, and `Delete`: plain
//! slices the author owns, the boilerplate the doctor would otherwise make them write six times. The entity's
//! writable scalar properties (`{ get; set; }`) are read by regex and spliced into the Create/Update inputs.
//! Value objects, enums, and other complex types cannot be built generically, so they are listed in one closing
//! note for the owner to finish. An existing slice is skipped, never clobbered, and the module's `Map` wiring is
//! best-effort: a missing anchor prints the exact line to add.
//!
//! No tests are emitted: what these slices must guarantee (tenant isolation, not-found on a foreign id) belongs in
//! a spec the author writes, with E2E cases that fail before the change.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::error_codes::{self, ErrorCode};
use super::{ApiProject, embedded, text};

/// Types taken straight from a request and assigned to a column. Anything else is "complex".
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

/// Columns the slice or the DbContext owns, never the request.
const SYSTEM_FIELDS: &[&str] = &["Id", "OrgId", "CreatedAt", "UpdatedAt", "UserId"];

static PROPERTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"public\s+(?<type>[A-Za-z0-9_<>,\.\? ]+?)\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)\s*\{\s*get;\s*set;\s*\}")
        .expect("property regex")
});

static GROUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"var\s+(?<g>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*app\.MapGroup").expect("group regex"));

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
        eprintln!("skies: no {entity} entity in {module} — create Modules/{module}/{entity}.cs first.");
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

    let (scalars, complex) = parse_fields(&source);
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

    wire_module(&module_file, &crud)?;
    summarize(&crud, &emitted);
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

    /// The structural tokens first (the longer `__ENTITY_LOWER__` before `__ENTITY__`), then the spliced field
    /// fragments, then the app tokens, exactly as the auth blueprint's token pass.
    fn render(&self, template: &str) -> String {
        let hyphen = text::hyphenate(&self.entity);
        let stamps_any = self.has_created_at || self.has_updated_at;
        let pick = |on: bool, value: &'static str| if on { value } else { "" };
        let body = text::fill(
            &text::normalize_newlines(template),
            &[
                ("__MODULE__", &self.module),
                ("__ENTITY_LOWER__", &hyphen),
                ("__ENTITY__", &self.entity),
                ("__PLURAL__", &self.plural),
                ("__CREATE_INPUT_FIELDS__", &self.create_input_fields()),
                ("__CREATE_ASSIGNMENTS__", &self.create_assignments()),
                ("__AUTH_USING__", pick(self.has_user_id, "using MyApp.Api.Auth;\n")),
                (
                    "__USERID_ASSIGNMENT__",
                    pick(self.has_user_id, "            UserId = current.UserId,\n"),
                ),
                ("__CURRENT_PARAM__", pick(self.has_user_id, "ICurrentUser current, ")),
                ("__CURRENT_ARG__", pick(self.has_user_id, "current, ")),
                ("__CREATE_TODO__", &self.todo("            ")),
                ("__UPDATE_TODO__", &self.todo("        ")),
                (
                    "__NOW_DECL__",
                    pick(stamps_any, "        var now = clock.GetUtcNow().UtcDateTime;\n"),
                ),
                (
                    "__CREATED_AT_ASSIGN__",
                    pick(self.has_created_at, "            CreatedAt = now,\n"),
                ),
                (
                    "__UPDATED_AT_ASSIGN__",
                    pick(self.has_updated_at, "            UpdatedAt = now,\n"),
                ),
                (
                    "__UPDATE_TOUCH__",
                    pick(
                        self.has_updated_at,
                        "        item.UpdatedAt = clock.GetUtcNow().UtcDateTime;\n",
                    ),
                ),
                ("__ORDER_KEY__", if self.has_created_at { "CreatedAt" } else { "Id" }),
                ("__UPDATE_INPUT_FIELDS__", &self.update_input_fields()),
                ("__UPDATE_ASSIGNMENTS__", &self.update_assignments()),
            ],
        );
        text::replace_app_tokens(&body, &self.app_name, &self.app_lower)
    }

    /// `string Name, int Age`. With no scalar fields the record still has to compile, so it gets a placeholder
    /// the route ignores.
    fn create_input_fields(&self) -> String {
        if self.scalars.is_empty() {
            return "string? Unused = null".to_string();
        }
        self.scalars
            .iter()
            .map(|f| format!("{} {}", f.ty, f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn create_assignments(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("            {0} = input.{0},\n", f.name))
            .collect()
    }

    /// Every scalar made nullable, defaulting to null, for a partial update.
    fn update_input_fields(&self) -> String {
        if self.scalars.is_empty() {
            return "string? Unused = null".to_string();
        }
        let nullable = |ty: &str| {
            if ty.ends_with('?') {
                ty.to_string()
            } else {
                format!("{ty}?")
            }
        };
        self.scalars
            .iter()
            .map(|f| format!("{} {} = null", nullable(&f.ty), f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A column is overwritten only when its input is non-null; a non-nullable value type unwraps with `.Value`.
    fn update_assignments(&self) -> String {
        self.scalars
            .iter()
            .map(|f| {
                let suffix = if f.ty.ends_with('?') || f.ty == "string" {
                    ""
                } else {
                    ".Value"
                };
                format!(
                    "        if (input.{0} is not null)\n            item.{0} = input.{0}{suffix};\n",
                    f.name
                )
            })
            .collect()
    }

    fn todo(&self, indent: &str) -> String {
        if self.complex.is_empty() {
            return String::new();
        }
        format!(
            "{indent}// TODO: set the complex/VO fields the generator could not auto-wire (see the note it printed).\n"
        )
    }
}

/// Reads the entity's `{ get; set; }` properties, splitting writable scalars from complex fields.
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
        r"DbSet<{}>\s+(?<plural>[A-Za-z_][A-Za-z0-9_]*)\s*=>",
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

/// Adds the missing `<Slice>.Map(<group>);` lines before the module's closing braces, using the group variable
/// the module declares (`var catalog = app.MapGroup(...)`), or `app` when there is none.
fn wire_module(module_file: &Path, crud: &Crud) -> Result<()> {
    let source = text::read(module_file)?;
    let nl = text::newline_of(&source);
    let group = GROUP
        .captures(&source)
        .map_or("app".to_string(), |c| c["g"].to_string());
    let missing: Vec<String> = crud
        .slices()
        .iter()
        .map(|slice| format!("        {slice}.Map({group});"))
        .filter(|line| !source.contains(line.trim()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    let anchor = format!("{nl}    }}{nl}}}");
    if source.contains(&anchor) {
        let block = format!("{nl}{}{anchor}", missing.join(nl));
        std::fs::write(module_file, text::replace_first(&source, &anchor, &block))?;
        println!("wired {} slice map(s) into {}Module.cs", missing.len(), crud.module);
    } else {
        for line in &missing {
            println!("note: add `{}` to {}Module.Map", line.trim(), crud.module);
        }
    }
    Ok(())
}

fn summarize(crud: &Crud, emitted: &[String]) {
    if !crud.has_user_id {
        println!(
            "note: {0} has no UserId — LookupMy{0} was not generated (the \"me\" lookup needs an owner column).",
            crud.entity
        );
    }
    if !crud.complex.is_empty() {
        let list: Vec<String> = crud.complex.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
        println!(
            "note: {} has complex/VO fields not auto-wired into Create/Update — add by hand: {}",
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
        "crud generated for {}/{} — {what}. Write down how it can fail in a spec (`skies spec new`), then prove it.",
        crud.module, crud.entity
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_scalar_fields_from_complex_and_system_ones() {
        let (scalars, complex) = parse_fields(
            "public Guid Id { get; set; }\npublic string Name { get; set; }\npublic int? Age { get; set; }\n\
             public Email Contact { get; set; }\npublic Guid OrgId { get; set; }\npublic string Slug { get; private set; }",
        );
        let names: Vec<_> = scalars.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
        assert_eq!(names, ["string Name", "int? Age"]);
        assert_eq!(complex.len(), 1);
        assert_eq!(complex[0].ty, "Email");
    }

    #[test]
    fn a_data_bag_entity_needs_the_tenant_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Acme.Api.csproj"), "<Project />").unwrap();
        std::fs::create_dir_all(dir.path().join("Modules/Catalog")).unwrap();
        std::fs::write(dir.path().join("Modules/Catalog/CatalogModule.cs"), "").unwrap();
        std::fs::write(
            dir.path().join("Modules/Catalog/Product.cs"),
            "public class Product { }",
        )
        .unwrap();

        assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
        assert!(!dir.path().join("Modules/Catalog/Slices").exists());
    }
}
