//! The shape generators: `g module`, `g slice`, `g entity`, `g vo`, and `g hub`.
//!
//! Each emits the conventional shape so the doctor passes by construction: a module owns both halves of its
//! wiring (SKY0015/16) and carries a ctx with its boundaries (SKY0004), a slice has the canonical
//! Input/Output/Handle/Map (SKY0001), a named endpoint (SKY0012), and an explicit authorization decision that
//! fails closed (SKY0022), an entity funnels every state through `EnsureValid` (SKY0014), and a value object is
//! only built through `From` (SKY0013). None of them writes a test: the behavior a slice must keep is described by a spec's
//! failure modes and proven by its E2E, written before the code.

use std::path::Path;

use anyhow::Result;

use super::error_codes::{self, ErrorCode};
use super::{ApiProject, embedded, text};

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

    // SKY0004 wants every module that owns a slice to carry a ctx with its boundaries; the skeleton gives the
    // owner the two sections to fill in, next to the code they describe.
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
    Ok(0)
}

/// Wires a module into `Modules/Modules.cs`, the explicit registry: `AddServices` before the `return services;`
/// that closes `AddModules`, `Map` after the last module's `Map` in `MapModules`, and the using. Skies discovers
/// nothing by reflection, so an unwired module is a silent 404; when the registry is missing or unusual the
/// generator says exactly which two lines to add.
fn wire_into_registry(project: &ApiProject, name: &str) -> Result<()> {
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

    let lower = name.to_lowercase();
    let body = text::fill(
        embedded::dotnet("scaffold/Slice.cs.cstmpl"),
        &[
            ("__NAMESPACE__", &project.namespace),
            ("__MODULE__", module),
            ("__NAME_LOWER__", &lower),
            ("__NAME__", name),
        ],
    );
    text::write(&path, body)?;
    println!("created {}", path.display());

    // The scaffolded validation references a registry constant (SKY0018), so the registry must declare it.
    let code = ErrorCode {
        name: "IdRequired",
        value: "id.required",
        summary: "The id input is required.",
    };
    error_codes::ensure(&project.module_dir(module), &project.namespace, module, &code)?;
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
    text::write(&path, body)?;
    println!("created {}", path.display());

    let code = ErrorCode {
        name: "IdRequired",
        value: "id.required",
        summary: "The id is required (entity invariant).",
    };
    error_codes::ensure(&module_dir, &project.namespace, module, &code)?;
    println!(
        "note: register it in AppDb.cs — add `public DbSet<{name}> {name}s => Set<{name}>();` — \
         then grow {name} with intention-revealing methods that funnel through EnsureValid."
    );
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
/// one-time `Program.cs` wiring rather than editing the composition root for a transport it cannot see.
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
        assert!(context.contains("## Boundaries\n\n- **Inside**") && context.contains("`BillingModule`"));
    }

    #[test]
    fn a_slice_brings_its_registry_constant() {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path());

        assert_eq!(slice(dir.path(), "Billing", "CreateInvoice").unwrap(), 0);

        let slice = std::fs::read_to_string(dir.path().join("Modules/Billing/Slices/CreateInvoice.cs")).unwrap();
        assert!(slice.contains("app.MapPost(\"/createinvoice\""));
        assert!(slice.contains("BillingErrorCodes.IdRequired"));
        assert!(slice.contains(".WithName(nameof(CreateInvoice))\n            .RequireAuthorization();"));
        assert!(dir.path().join("Modules/Billing/BillingErrorCodes.cs").exists());
        assert!(
            !dir.path()
                .join("Modules/Billing/Slices/CreateInvoice.Tests.cs")
                .exists()
        );
    }

    #[test]
    fn generators_need_a_project() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(value_object(dir.path(), "Money").unwrap(), 1);
    }
}
