//! The shape generators: `g module`, `g slice`, `g entity`, `g vo`, and `g hub`.
//!
//! Each emits the conventional shape so the doctor passes by construction: a module owns both halves of its
//! wiring (SKY0015/16), a slice has the canonical Input/Output/Handle/Map (SKY0001), a named endpoint (SKY0012),
//! is mapped under its module's route group, and takes the group's authorization decision or states its own,
//! failing closed (SKY0022), an entity funnels every state through `EnsureValid` (SKY0014), and a value object is
//! only built through `From` (SKY0013). One thing is left failing on purpose: a module's ctx skeleton carries its
//! hints as HTML comments, so SKY0004 asks the author for the module's boundaries and design notes once the module
//! owns a slice; no generator writes prose that would pass for them. None of them writes a test: the behavior a
//! slice must keep is described by a spec's failure modes and proven by its E2E, written before the code. Nothing
//! they write into the app cites a rule id: generated comments explain the domain, not the linter.

use std::path::Path;

use anyhow::Result;

use super::app_db;
use super::crud::module as group;
use super::error_codes::{self, ErrorCode};
use super::{ApiProject, embedded, text};

/// The end of a slice's `Map` chain when the module's route group decides authorization for it.
pub(super) const INHERITED_POSTURE: &str = ";";

/// The end of a slice's `Map` chain when nothing above it decides: fail closed.
pub(super) const OWN_POSTURE: &str = "\n            .RequireAuthorization();";

pub fn module(root: &Path, name: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let path = project.module_dir(name).join(format!("{name}Module.cs"));
    if path.exists() {
        eprintln!("skies: {} already exists.", path.display());
        return Ok(1);
    }

    let lower = name.to_lowercase();
    let body = text::fill(
        embedded::dotnet("scaffold/Module.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__NAME_LOWER__", &lower),
            ("__NAME__", name),
        ],
    );
    text::write(&path, body)?;
    println!("created {}", path.display());

    // Only the author knows the module's why; the skeleton carries the two spine sections with their hints
    // commented out, so the build asks for them (SKY0004) instead of accepting generated prose.
    let context = project.module_dir(name).join(format!("{name}.ctx.md"));
    if !context.exists() {
        let body = text::fill(
            embedded::dotnet("scaffold/Module.ctx.md.cstmpl"),
            &[("__NAME_LOWER__", &lower), ("__NAME__", name)],
        );
        text::write(&context, body)?;
        println!("created {}", context.display());
    }

    wire_into_registry(&project, name)?;
    println!(
        "next: write {name}'s boundaries and design notes in {}. The build reports SKY0004 until both sections \
         hold your own words (the commented hints do not count).",
        context.display()
    );
    Ok(0)
}

/// Wires a module into `Modules/Modules.cs`, the explicit registry: `AddServices` before the `return services;`
/// that closes `AddModules`, `Map` after the last module's `Map` in `MapModules`, and the using. Skies discovers
/// nothing by reflection, so an unwired module is a silent 404; when the registry is missing or unusual the
/// generator says exactly which two lines to add.
pub(super) fn wire_into_registry(project: &ApiProject, name: &str) -> Result<()> {
    let note = format!(
        "note: wire the module — add {name}Module.AddServices(services, configuration); to AddModules \
         and {name}Module.Map(app); to MapModules (Modules/Modules.cs)."
    );
    let registry = project.root.join("Modules").join("Modules.cs");
    if !registry.exists() {
        println!("{note}");
        return Ok(());
    }

    let mut source = text::read(&registry)?;
    if source.contains(&format!("{name}Module.Map")) {
        return Ok(());
    }
    if !source.contains("return services;") || !source.contains(".Map(app);") {
        println!("{note}");
        return Ok(());
    }

    let using = format!("using {}.Modules.{name};", project.namespace);
    if !source.contains(&using) {
        source = format!("{using}\n{source}");
    }
    source = source.replace(
        "        return services;",
        &format!("        {name}Module.AddServices(services, configuration);\n        return services;"),
    );

    let map_line = format!("        {name}Module.Map(app);\n");
    let last_map = source.rfind(".Map(app);").unwrap_or(0);
    match source[last_map..].find('\n') {
        Some(offset) => source.insert_str(last_map + offset + 1, &map_line),
        None => source.push_str(&format!("\n{map_line}")),
    }

    std::fs::write(&registry, source)?;
    println!("wired {name}Module into Modules/Modules.cs");
    Ok(())
}

pub fn slice(root: &Path, module: &str, name: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let path = project.module_dir(module).join("Slices").join(format!("{name}.cs"));
    if path.exists() {
        eprintln!("skies: {} already exists.", path.display());
        return Ok(1);
    }

    // The module's group decides for every slice mapped on it; a slice restates a posture only when nothing above
    // it does (no module file yet, a group without one, or a module the generator cannot wire).
    let module_file = project.module_dir(module).join(format!("{module}Module.cs"));
    let inherits = module_file.is_file() && group::group_decides(&text::read(&module_file)?);
    let posture = if inherits { INHERITED_POSTURE } else { OWN_POSTURE };
    let lower = name.to_lowercase();
    let body = text::fill(
        embedded::dotnet("scaffold/Slice.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__MODULE__", module),
            ("__NAME_LOWER__", &lower),
            ("__NAME__", name),
            ("__POSTURE__", posture),
        ],
    );
    text::write(&path, body)?;
    println!("created {}", path.display());

    // The slice is live on purpose: mapped and in the OpenAPI contract, so `g client` and `g feature` can bind a
    // screen to it before its behavior exists. It answers an honest "not implemented" business error instead of fake
    // behavior, so every spec case against it fails red until the operation is written; the code lives on the
    // registry like any other (SKY0018).
    let not_implemented = format!("{name}NotImplemented");
    let value = format!(
        "{}.{}_not_implemented",
        module.to_lowercase(),
        text::hyphenate(name).replace('-', "_")
    );
    let summary = format!("{name} is scaffolded but not implemented yet; remove this code when it is.");
    let code = ErrorCode {
        name: &not_implemented,
        value: &value,
        summary: &summary,
    };
    error_codes::ensure(&project.module_dir(module), &project.namespace, module, &code)?;

    // An unmapped slice is a silent 404: map it under the module's group, as `g crud` does.
    if module_file.is_file() {
        group::wire(&module_file, module, &[name.to_string()])?;
    } else {
        println!(
            "note: {} does not exist; run `skies g module {module}`, then add `{name}.Map(<group>);` to \
             {module}Module.Map.",
            module_file.display()
        );
    }
    println!(
        "next: {name} is mapped and answers {module}ErrorCodes.{not_implemented} until you write its Input, Output, \
         and Handle, so a spec's E2E against it starts red. Remove the code once it is implemented."
    );
    Ok(0)
}

pub fn entity(root: &Path, module: &str, name: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let module_dir = project.module_dir(module);
    let path = module_dir.join(format!("{name}.cs"));
    if path.exists() {
        eprintln!("skies: {} already exists.", path.display());
        return Ok(1);
    }

    let body = text::fill(
        embedded::dotnet("scaffold/Entity.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__MODULE__", module),
            ("__NAME__", name),
        ],
    );
    // A multi-tenant app scopes an entity by adding `ITenantScoped` to it: the import is already there, so that one
    // edit compiles.
    let tenancy = text::read(&project.csproj)?.contains("\"Skies.Framework.EntityFrameworkCore\"");
    let body = if tenancy {
        format!("using Skies.Framework.EntityFrameworkCore;{}{}{body}", text::newline_of(&body), text::newline_of(&body))
    } else {
        body
    };
    text::write(&path, body)?;
    println!("created {}", path.display());

    let value = format!("{}.id_required", module.to_lowercase());
    let code = ErrorCode {
        name: "IdRequired",
        value: &value,
        summary: "The id is required (entity invariant).",
    };
    error_codes::ensure(&module_dir, &project.namespace, module, &code)?;
    let set = format!("public DbSet<{name}> {} => Set<{name}>();", text::plural(name));
    match app_db::locate(&project.root)? {
        Some(file) => println!(
            "note: register it in {} with `{set}` (`skies g crud {module} {name}` does that for you), then grow \
             {name} with intention-revealing methods that funnel through EnsureValid.",
            file.display()
        ),
        None => println!(
            "note: this project has no AppDb yet (`skies g auth` adds one). Register {name} in the DbContext your \
             slices take as AppDb with `{set}`, then grow it with intention-revealing methods that funnel through \
             EnsureValid."
        ),
    }
    Ok(0)
}

/// Value objects are generic, so they land in `BuildingBlocks/`; a module-specific one can be moved by hand.
pub fn value_object(root: &Path, name: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let path = project.root.join("BuildingBlocks").join(format!("{name}.cs"));
    if path.exists() {
        eprintln!("skies: {} already exists.", path.display());
        return Ok(1);
    }

    let lower = name.to_lowercase();
    let body = text::fill(
        embedded::dotnet("scaffold/ValueObject.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__NAME_LOWER__", &lower),
            ("__NAME__", name),
        ],
    );
    text::write(&path, body)?;
    println!("created {}", path.display());
    println!("note: fill in {name}.From's invariant, then prefer {name} over the raw type in your slices.");
    Ok(0)
}

/// Real-time is opt-in: a fresh app carries no hub until one is generated, and the generator prints the
/// one-time wiring for the module rather than editing its composition for a transport it cannot see.
pub fn hub(root: &Path, module: &str, name: &str) -> Result<u8> {
    let Some(project) = ApiProject::open(root)? else {
        return Ok(1);
    };
    let path = project
        .module_dir(module)
        .join("Realtime")
        .join(format!("{name}Hub.cs"));
    if path.exists() {
        eprintln!("skies: {} already exists.", path.display());
        return Ok(1);
    }

    let body = text::fill(
        embedded::dotnet("scaffold/Hub.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__MODULE__", module),
            ("__NAME__", name),
        ],
    );
    text::write(&path, body)?;
    println!("created {}", path.display());

    let lower = name.to_lowercase();
    let wiring = text::fill(
        embedded::dotnet("scaffold/HubWiring.txt"),
        &[("__NAME_LOWER__", &lower), ("__NAME__", name)],
    );
    print!("{wiring}");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(dir: &Path) {
        std::fs::write(dir.join("Acme.Api.csproj"), "<Project />\n").unwrap();
        std::fs::create_dir_all(dir.join("Modules")).unwrap();
        std::fs::write(
            dir.join("Modules/Modules.cs"),
            "using Acme.Api.Modules.Health;\n\npublic static class Modules\n{\n    public static IServiceCollection \
             AddModules(this IServiceCollection services, IConfiguration configuration)\n    {\n        \
             HealthModule.AddServices(services, configuration);\n        return services;\n    }\n\n    public static \
             void MapModules(this WebApplication app)\n    {\n        HealthModule.Map(app);\n    }\n}\n",
        )
        .unwrap();
    }

    #[test]
    fn a_module_is_wired_on_both_sides_of_the_registry_once() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());

        assert_eq!(module(dir.path(), "Billing").unwrap(), 0);
        assert_eq!(
            module(dir.path(), "Billing").unwrap(),
            1,
            "a second run refuses to clobber"
        );

        let registry = std::fs::read_to_string(dir.path().join("Modules/Modules.cs")).unwrap();
        assert!(registry.starts_with("using Acme.Api.Modules.Billing;\nusing Acme.Api.Modules.Health;"));
        assert!(registry.contains(
            "HealthModule.AddServices(services, configuration);\n        BillingModule.AddServices(services, configuration);\n        return services;"
        ));
        assert!(registry.contains("HealthModule.Map(app);\n        BillingModule.Map(app);\n    }"));
        let context = std::fs::read_to_string(dir.path().join("Modules/Billing/Billing.ctx.md")).unwrap();
        assert!(context.contains("## Boundaries\n\n<!-- Inside: "));
        let prose: Vec<&str> = context
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("<!--"))
            .collect();
        assert!(
            prose.is_empty(),
            "the skeleton writes no prose that could pass for the author's: {prose:?}"
        );
        let wiring = read(dir.path(), "Modules/Billing/BillingModule.cs");
        assert!(
            !wiring.contains("        //"),
            "no commented example is left for the author to clean up"
        );
        assert!(!wiring.contains("SKY"));
    }

    #[test]
    fn a_slice_brings_its_registry_constant() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());

        assert_eq!(slice(dir.path(), "Billing", "CreateInvoice").unwrap(), 0);

        let slice = std::fs::read_to_string(dir.path().join("Modules/Billing/Slices/CreateInvoice.cs")).unwrap();
        assert!(slice.contains("app.MapPost(\"/createinvoice\""));
        assert!(slice.contains(
            "Error.BusinessRule(BillingErrorCodes.CreateInvoiceNotImplemented, \"CreateInvoice is not implemented yet.\")"
        ));
        assert!(!slice.contains("new Output(input") && !slice.contains("fill in") && !slice.contains("SKY"));
        let codes = read(dir.path(), "Modules/Billing/BillingErrorCodes.cs");
        assert!(codes.contains("CreateInvoiceNotImplemented = \"billing.create_invoice_not_implemented\";"));
        assert!(slice.contains(".WithName(nameof(CreateInvoice))\n            .RequireAuthorization();"));
        assert!(dir.path().join("Modules/Billing/BillingErrorCodes.cs").exists());
        assert!(
            !dir.path()
                .join("Modules/Billing/Slices/CreateInvoice.Tests.cs")
                .exists()
        );
    }

    fn module_with_map(dir: &Path, map_body: &str) {
        let file = dir.join("Modules/Wallets/WalletsModule.cs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            file,
            format!(
                "public static class WalletsModule\n{{\n    public static void Map(IEndpointRouteBuilder app)\n    \
                 {{\n{map_body}    }}\n}}\n"
            ),
        )
        .unwrap();
    }

    fn read(dir: &Path, path: &str) -> String {
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    #[test]
    fn a_slice_under_an_anonymous_group_is_wired_and_states_no_posture_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());
        module_with_map(
            dir.path(),
            "        var wallets = app.MapGroup(\"/wallets\").AllowAnonymous();\n        Deposit.Map(wallets);\n",
        );

        assert_eq!(slice(dir.path(), "Wallets", "Transfer").unwrap(), 0);

        let slice = read(dir.path(), "Modules/Wallets/Slices/Transfer.cs");
        assert!(slice.contains(".WithName(nameof(Transfer));\n"));
        assert!(!slice.contains("RequireAuthorization"));
        let module = read(dir.path(), "Modules/Wallets/WalletsModule.cs");
        assert!(module.contains("        Deposit.Map(wallets);\n        Transfer.Map(wallets);\n    }\n}\n"));
    }

    #[test]
    fn a_fresh_module_gets_a_fail_closed_group_the_slice_inherits() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());
        assert_eq!(module(dir.path(), "Billing").unwrap(), 0);

        assert_eq!(slice(dir.path(), "Billing", "CreateInvoice").unwrap(), 0);

        let module = read(dir.path(), "Modules/Billing/BillingModule.cs");
        assert!(module.contains(
            "        var billing = app.MapGroup(\"/billing\").RequireAuthorization();\n        \
             CreateInvoice.Map(billing);\n    }\n}\n"
        ));
        let slice = read(dir.path(), "Modules/Billing/Slices/CreateInvoice.cs");
        assert!(slice.contains(".WithName(nameof(CreateInvoice));"));
        assert!(!slice.contains("RequireAuthorization"));
    }

    #[test]
    fn a_group_without_a_posture_leaves_the_slice_failing_closed() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());
        module_with_map(dir.path(), "        var wallets = app.MapGroup(\"/wallets\");\n");

        assert_eq!(slice(dir.path(), "Wallets", "Transfer").unwrap(), 0);

        let slice = read(dir.path(), "Modules/Wallets/Slices/Transfer.cs");
        assert!(slice.contains(".WithName(nameof(Transfer))\n            .RequireAuthorization();"));
        assert!(read(dir.path(), "Modules/Wallets/WalletsModule.cs").contains("Transfer.Map(wallets);"));
    }

    #[test]
    fn the_entity_note_names_the_real_db_context() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());
        assert_eq!(entity(dir.path(), "Billing", "Invoice").unwrap(), 0);
        let source = read(dir.path(), "Modules/Billing/Invoice.cs");
        assert!(!source.contains("SKY") && !source.contains(" a Invoice"));
        assert!(
            !source.contains("updates validate"),
            "no comment promises an update the entity does not have"
        );
        let codes = read(dir.path(), "Modules/Billing/BillingErrorCodes.cs");
        assert!(codes.contains("public const string IdRequired = \"billing.id_required\";"));
    }

    #[test]
    fn generators_need_a_project() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(value_object(dir.path(), "Money").unwrap(), 1);
    }
}
