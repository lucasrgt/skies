//! The module half of `g crud` and `g slice`: mapping the slices under the module's route group.
//!
//! The slices map onto the group the module declares (`var catalog = app.MapGroup(...)`), which carries the
//! module's authorization decision. The group is matched on code lines only, so a commented-out example never
//! counts; when the module has none (a fresh `g module` scaffold), the generator declares it, failing closed
//! (`RequireAuthorization`), which is the decision SKY0022 wants explicit.
//!
//! Writes to an app-wide entity get a second group over the same prefix, `<group>Admin`, declared in the module
//! beside the first so the decision is read where every other one is: the auth blueprint's `AppPolicies.AppAdmin`
//! when the app has it, and a policy no one satisfies when it does not. It is a group of its own, not a nested one,
//! because an `.AllowAnonymous()` on the module group would otherwise waive the admin requirement too.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::text;

/// A group declared on a line of code: leading whitespace only, so `//   var x = app.MapGroup(...)` never
/// matches.
static GROUP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*var\s+(?<g>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*app\.MapGroup\b").expect("group regex")
});

/// The group variable the module already declares, if any.
fn declared_group(source: &str) -> Option<String> {
    GROUP.captures(source).map(|c| c["g"].to_string())
}

/// The end of `Map` and of the class in a scaffolded module: where new lines go.
fn anchor(nl: &str) -> String {
    format!("{nl}    }}{nl}}}")
}

/// Whether slices mapped by [`wire`] inherit an authorization decision (SKY0022) from the module's group: the group
/// statement carries `.AllowAnonymous()` or `.RequireAuthorization(…)`, or the module has no group and `wire` will
/// declare one, failing closed, at the scaffold's anchor. A slice that inherits must not restate a posture: a
/// per-endpoint `.RequireAuthorization()` under an `.AllowAnonymous()` group demands an auth scheme the app may
/// not have.
pub(crate) fn group_decides(source: &str) -> bool {
    match GROUP.find(source) {
        Some(found) => {
            let rest = &source[found.start()..];
            let statement = &rest[..rest.find(';').unwrap_or(rest.len())];
            statement.contains(".AllowAnonymous()") || statement.contains(".RequireAuthorization(")
        }
        None => source.contains(&anchor(text::newline_of(source))),
    }
}

/// Adds the missing `<Slice>.Map(<group>);` lines before the module's closing braces, declaring the group first
/// when the module has none. A module without the scaffold's closing anchor gets the exact lines to add.
pub(crate) fn wire(module_file: &Path, module: &str, slices: &[String]) -> Result<()> {
    let source = text::read(module_file)?;
    let nl = text::newline_of(&source);
    let (group, declaration) = match declared_group(&source) {
        Some(group) => (group, None),
        None => {
            let group = module.to_lowercase();
            let line = format!("        var {group} = app.MapGroup(\"/{group}\").RequireAuthorization();");
            (group, Some(line))
        }
    };
    let maps: Vec<String> = slices
        .iter()
        .map(|slice| format!("        {slice}.Map({group});"))
        .filter(|line| !source.contains(line.trim()))
        .collect();
    if maps.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = declaration.into_iter().chain(maps).collect();

    let anchor = anchor(nl);
    if source.contains(&anchor) {
        let block = format!("{nl}{}{anchor}", lines.join(nl));
        std::fs::write(module_file, text::replace_first(&source, &anchor, &block))?;
        println!("wired {} line(s) into {module}Module.Map", lines.len());
    } else {
        for line in &lines {
            println!("note: add `{}` to {module}Module.Map", line.trim());
        }
    }
    Ok(())
}

/// A group's route prefix: `/catalog` in `var catalog = app.MapGroup("/catalog")...`.
fn group_prefix(source: &str, group: &str) -> Option<String> {
    let pattern = format!(
        r#"var\s+{}\s*=\s*app\.MapGroup\(\s*"(?<prefix>[^"]*)""#,
        regex::escape(group)
    );
    Regex::new(&pattern)
        .expect("group prefix regex")
        .captures(source)
        .map(|c| c["prefix"].to_string())
}

/// Maps `slices` (the writes to an app-wide entity) under the module's admin group, declaring it once after the
/// module group with the policy `app_admin` says the app has. Run after [`wire`], which guarantees the module group.
pub(crate) fn wire_admin(module_file: &Path, module: &str, slices: &[String], app_admin: bool) -> Result<()> {
    let source = text::read(module_file)?;
    let nl = text::newline_of(&source);
    let Some(group) = declared_group(&source) else {
        println!(
            "note: declare {module}Module's route group, then map {} under an admin group.",
            slices.join(", ")
        );
        return Ok(());
    };
    let admin = format!("{group}Admin");
    let declaration = if source.contains(&format!("var {admin} =")) {
        None
    } else {
        let Some(prefix) = group_prefix(&source, &group) else {
            println!(
                "note: map {} under a group of {module}Module's prefix that requires AppPolicies.AppAdmin.",
                slices.join(", ")
            );
            return Ok(());
        };
        Some(admin_group(&admin, &prefix, app_admin, nl))
    };
    let maps: Vec<String> = slices
        .iter()
        .map(|slice| format!("        {slice}.Map({admin});"))
        .filter(|line| !source.contains(line.trim()))
        .collect();
    if maps.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = declaration.into_iter().chain(maps).collect();
    let anchor = anchor(nl);
    if source.contains(&anchor) {
        let block = format!("{nl}{}{anchor}", lines.join(nl));
        std::fs::write(module_file, text::replace_first(&source, &anchor, &block))?;
        println!("wired {} admin line(s) into {module}Module.Map", lines.len());
    } else {
        for line in &lines {
            println!("note: add `{}` to {module}Module.Map", line.trim());
        }
    }
    Ok(())
}

/// The admin group's declaration, with the comment that states the decision it makes.
fn admin_group(admin: &str, prefix: &str, app_admin: bool, nl: &str) -> String {
    if app_admin {
        [
            "        // App-wide data is shared by every user: only an app admin may change it.".to_string(),
            format!("        var {admin} = app.MapGroup(\"{prefix}\").RequireAuthorization(AppPolicies.AppAdmin);"),
        ]
        .join(nl)
    } else {
        [
            "        // App-wide data is shared by every user, and this app names no admin yet: its writes stay closed"
                .to_string(),
            "        // to everyone until this requires the policy of whoever may change it.".to_string(),
            format!(
                "        var {admin} = app.MapGroup(\"{prefix}\").RequireAuthorization(policy => policy.RequireAssertion(_ => false));"
            ),
        ]
        .join(nl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCAFFOLD: &str = concat!(
        "    public static void Map(IEndpointRouteBuilder app)\n    {\n",
        "        // Register slices:\n",
        "        //   var catalog = app.MapGroup(\"/catalog\").RequireAuthorization(); // or .AllowAnonymous()\n",
        "        //   <Slice>.Map(catalog);\n",
        "    }\n}\n",
    );

    #[test]
    fn a_commented_example_is_not_a_group() {
        assert_eq!(declared_group(SCAFFOLD), None);
        assert_eq!(
            declared_group("        var shop = app.MapGroup(\"/shop\").AllowAnonymous();\n").as_deref(),
            Some("shop")
        );
    }

    #[test]
    fn a_module_without_a_group_gets_one_before_its_maps_and_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("CatalogModule.cs");
        std::fs::write(&file, SCAFFOLD).unwrap();
        let slices = vec!["ListProducts".to_string(), "CreateProduct".to_string()];

        wire(&file, "Catalog", &slices).unwrap();
        wire(&file, "Catalog", &slices).unwrap();

        let wired = std::fs::read_to_string(&file).unwrap();
        assert!(wired.ends_with(concat!(
            "//   <Slice>.Map(catalog);\n",
            "        var catalog = app.MapGroup(\"/catalog\").RequireAuthorization();\n",
            "        ListProducts.Map(catalog);\n",
            "        CreateProduct.Map(catalog);\n",
            "    }\n}\n",
        )));
        assert_eq!(
            wired.matches("var catalog").count(),
            2,
            "the comment and one declaration"
        );
    }

    #[test]
    fn a_group_posture_is_inherited_only_when_the_group_states_one() {
        assert!(group_decides(
            "        var w = app.MapGroup(\"/w\").AllowAnonymous();\n"
        ));
        assert!(group_decides(
            "        var w = app.MapGroup(\"/w\")\n            .RequireAuthorization(\"admin\");\n"
        ));
        assert!(!group_decides(
            "        var w = app.MapGroup(\"/w\");\n        w.WithTags(\"x\");\n    }\n}\n"
        ));
        assert!(
            group_decides(SCAFFOLD),
            "wire declares a fail-closed group at the anchor"
        );
        assert!(!group_decides(
            "    public static void Map(IEndpointRouteBuilder app) { }\n}\n"
        ));
    }

    #[test]
    fn an_existing_group_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ShopModule.cs");
        std::fs::write(
            &file,
            concat!(
                "    {\n",
                "        var shop = app.MapGroup(\"/shop\").AllowAnonymous();\n",
                "        Browse.Map(shop);\n",
                "    }\n}\n",
            ),
        )
        .unwrap();

        wire(&file, "Shop", &["ListProducts".to_string()]).unwrap();

        let wired = std::fs::read_to_string(&file).unwrap();
        assert!(wired.contains("Browse.Map(shop);\n        ListProducts.Map(shop);\n    }"));
        assert_eq!(wired.matches("MapGroup").count(), 1);
    }

    #[test]
    fn app_wide_writes_get_one_admin_group_over_the_module_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ShopModule.cs");
        std::fs::write(
            &file,
            "    {\n        var shop = app.MapGroup(\"/shop\").AllowAnonymous();\n        ListTags.Map(shop);\n    }\n}\n",
        )
        .unwrap();

        wire_admin(&file, "Shop", &["CreateTag".to_string()], true).unwrap();
        wire_admin(&file, "Shop", &["CreateTag".to_string(), "DeleteTag".to_string()], true).unwrap();

        let wired = std::fs::read_to_string(&file).unwrap();
        assert!(wired.contains(concat!(
            "        var shopAdmin = app.MapGroup(\"/shop\").RequireAuthorization(AppPolicies.AppAdmin);\n",
            "        CreateTag.Map(shopAdmin);\n",
            "        DeleteTag.Map(shopAdmin);\n    }\n}\n",
        )));
        assert_eq!(wired.matches("var shopAdmin").count(), 1);
    }

    #[test]
    fn without_an_admin_policy_app_wide_writes_are_closed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ShopModule.cs");
        std::fs::write(
            &file,
            "    {\n        var shop = app.MapGroup(\"/shop\").RequireAuthorization();\n    }\n}\n",
        )
        .unwrap();

        wire_admin(&file, "Shop", &["CreateTag".to_string()], false).unwrap();

        let wired = std::fs::read_to_string(&file).unwrap();
        assert!(wired.contains("RequireAuthorization(policy => policy.RequireAssertion(_ => false));"));
        assert!(!wired.contains("AppPolicies"));
    }
}
