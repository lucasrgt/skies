//! Generator snapshots: replay `skies new` and every `skies g` generator into a temporary solution and compare the
//! tree byte for byte with `fixtures/snapshots/<tree>`. The snapshots were first checked against the 4.x CLI's
//! output (every difference was deliberate: no proof ceremony, doctor-clean crud and slices, auth specs).
//!
//! After an intended template change, re-bless with `SKIES_BLESS=1 cargo test --test generators` and review the
//! fixture diff in the commit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn skies(dir: &Path, args: &[&str]) {
    let status = Command::new(env!("CARGO_BIN_EXE_skies"))
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("run skies");
    assert!(status.success(), "skies {} failed in {}", args.join(" "), dir.display());
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn snapshot(tree: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/snapshots")
        .join(tree)
}

/// Compares a generated solution with its snapshot, or rewrites the snapshot when `SKIES_BLESS` is set.
fn assert_snapshot(tree: &str, generated: &Path) {
    let expected_root = snapshot(tree);
    if std::env::var_os("SKIES_BLESS").is_some() {
        let _ = std::fs::remove_dir_all(&expected_root);
        for (path, bytes) in files(generated) {
            let target = expected_root.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
        return;
    }
    let expected = files(&expected_root);
    let ours = files(generated);
    let mut mismatches = Vec::new();
    for (path, bytes) in &expected {
        match ours.get(path) {
            None => mismatches.push(format!("missing: {path}")),
            Some(our_bytes) if our_bytes != bytes => mismatches.push(format!("differs: {path}")),
            Some(_) => {}
        }
    }
    mismatches.extend(
        ours.keys()
            .filter(|path| !expected.contains_key(*path))
            .map(|path| format!("unexpected: {path}")),
    );
    assert!(
        mismatches.is_empty(),
        "{tree} diverges from its snapshot (re-bless with SKIES_BLESS=1 if intended):\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn the_full_generator_sequence_matches_its_snapshot() {
    let work = tempfile::tempdir().unwrap();
    skies(work.path(), &["new", "Golden"]);
    let solution = work.path().join("Golden");
    let api = solution.join("src/Golden.Api");
    for args in [
        &["g", "module", "Billing"][..],
        &["g", "slice", "Billing", "CreateInvoice"],
        &["g", "slice", "Billing", "GetInvoice"],
        &["g", "entity", "Billing", "Invoice"],
        &["g", "vo", "Money"],
        &["g", "module", "Catalog"],
        &["g", "hub", "Billing", "Payments"],
        &["g", "auth"],
        &["g", "auth:otp"],
        &["g", "auth:oauth"],
        &["g", "auth:email"],
        &["g", "entity", "Catalog", "Product"],
    ] {
        skies(&api, args);
    }

    // The owner's part before crud: an ITenantScoped entity with domain state.
    let product = api.join("Modules/Catalog/Product.cs");
    let source = std::fs::read_to_string(&product).unwrap();
    let edited = source.replacen("public class Product\n", "public class Product : ITenantScoped\n", 1).replacen(
        "    public Guid Id { get; private set; }\n",
        "    public Guid Id { get; private set; }\n\n    public Guid OrgId { get; private set; }\n    public string Name { get; private set; } = \"\";\n",
        1,
    );
    std::fs::write(&product, edited).unwrap();
    skies(&api, &["g", "crud", "Catalog", "Product"]);

    assert_snapshot("Golden", &solution);
    crud_output_is_doctor_shaped(&api);
}

/// The properties the crud snapshot must keep however it is re-blessed: no slice writes a column, the entity
/// carries the members the slices call, and the module maps them under a real (uncommented) group.
fn crud_output_is_doctor_shaped(api: &Path) {
    let read = |path: &str| std::fs::read_to_string(api.join(path)).unwrap();
    let product = read("Modules/Catalog/Product.cs");
    assert!(
        !product.contains("{ get; set; }"),
        "an [Entity] has no public setter (SKY0014)"
    );
    assert!(product.contains(
        "    public static Result<Product> Open(Guid id, string name) =>\n        new Product { Id = id, Name = name }.EnsureValid();\n"
    ));
    assert!(product.contains("proposed.Name = name;"));
    assert!(product.contains("if (validation.IsFailure) return validation.Error;\n        Name = proposed.Name;"));
    assert!(product.contains(
        "[System.ComponentModel.DataAnnotations.Timestamp]\n    public byte[]? RowVersion { get; private set; }"
    ));

    let create = read("Modules/Catalog/Slices/CreateProduct.cs");
    assert!(create.contains("var opened = Product.Open(Guid.NewGuid(), input.Name);"));
    assert!(!create.contains("new Product"));
    let update = read("Modules/Catalog/Slices/UpdateProduct.cs");
    assert!(update.contains("var updated = item.Update(input.Name);\n        if (updated.IsFailure)"));
    let list = read("Modules/Catalog/Slices/ListProduct.cs");
    assert!(list.contains(
        "db.Products.OrderBy(e => e.Id)\n            .ToPageAsync(input.Page, input.PageSize, MaxPageSize, ct);"
    ));
    for slice in ["List", "Lookup", "Create", "Update", "Delete"] {
        let source = read(&format!("Modules/Catalog/Slices/{slice}Product.cs"));
        assert!(source.contains(&format!(
            ".WithName(nameof({slice}Product))\n            .RequireAuthorization();"
        )));
    }

    let module = read("Modules/Catalog/CatalogModule.cs");
    assert!(module.contains(
        "        var catalog = app.MapGroup(\"/catalog\").RequireAuthorization();\n        ListProduct.Map(catalog);\n"
    ));
}

#[test]
fn the_auth_variants_match_their_snapshots() {
    for (tree, flag) in [
        ("G-skip-tenancy", "--skip-tenancy"),
        ("G-skip-cookies", "--skip-cookies"),
    ] {
        let work = tempfile::tempdir().unwrap();
        skies(work.path(), &["new", "Golden"]);
        let solution = work.path().join("Golden");
        skies(&solution.join("src/Golden.Api"), &["g", "auth", flag]);
        assert_snapshot(tree, &solution);
    }
}

#[test]
fn backend_generators_run_from_the_app_root_and_the_manual_names_the_app() {
    let work = tempfile::tempdir().unwrap();
    skies(work.path(), &["new", "Golden"]);
    let root = work.path().join("Golden");
    skies(&root, &["g", "module", "Billing"]);
    skies(&root, &["g", "slice", "Billing", "CreateInvoice"]);
    skies(
        &root.join("src"),
        &["g", "slice", "Billing", "GetInvoice", "--project", "Golden.Api"],
    );

    let api = root.join("src/Golden.Api");
    let module = std::fs::read_to_string(api.join("Modules/Billing/BillingModule.cs")).unwrap();
    assert!(module.contains(
        "        var billing = app.MapGroup(\"/billing\").RequireAuthorization();\n        \
         CreateInvoice.Map(billing);\n        GetInvoice.Map(billing);\n    }\n}"
    ));
    assert!(api.join("Modules/Billing/Slices/GetInvoice.cs").is_file());
    assert!(!root.join("Modules").exists() && !root.join("src/Modules").exists());

    // Every rendered file names the app, never the template's source name.
    for (path, bytes) in files(&root) {
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !path.contains("Starter") && !text.contains("Skies.Framework.Starter"),
            "{path} leaks the template"
        );
    }
    let manual = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(manual.contains("`src/Golden.Api`") && manual.contains("`tests/Golden.Tests`"));
}

#[test]
fn crud_refuses_an_entity_that_is_not_tenant_scoped() {
    let work = tempfile::tempdir().unwrap();
    skies(work.path(), &["new", "Golden"]);
    let api = work.path().join("Golden/src/Golden.Api");
    skies(&api, &["g", "module", "Catalog"]);
    skies(&api, &["g", "entity", "Catalog", "Product"]);

    let output = Command::new(env!("CARGO_BIN_EXE_skies"))
        .args(["g", "crud", "Catalog", "Product"])
        .current_dir(&api)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Product is not ITenantScoped"));
    assert!(!api.join("Modules/Catalog/Slices").exists());
}

#[test]
fn auth_refuses_to_overwrite_owner_files_before_writing_anything() {
    for relative in [
        "src/Golden.Api/AppDb.cs",
        "src/Golden.Api/AppJson.cs",
        "src/Golden.Api/Platform.cs",
        "tests/Golden.Tests/TestApp.cs",
    ] {
        let work = tempfile::tempdir().unwrap();
        skies(work.path(), &["new", "Golden"]);
        let root = work.path().join("Golden");
        std::fs::write(root.join(relative), "// application-owned code\n").unwrap();
        let before = files(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_skies"))
            .args(["g", "auth"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("no files were changed"));
        assert_eq!(files(&root), before, "{relative}");
    }
}

#[test]
fn auth_uses_the_same_module_registry_as_other_features() {
    let work = tempfile::tempdir().unwrap();
    skies(work.path(), &["new", "Golden"]);
    let root = work.path().join("Golden");
    skies(&root, &["g", "auth"]);
    let api = root.join("src/Golden.Api");
    let registry = std::fs::read_to_string(api.join("Modules/Modules.cs")).unwrap();
    assert!(registry.contains("AccountModule.AddServices(services, configuration)"));
    assert!(registry.contains("AccountModule.Map(app)"));
    let program = std::fs::read_to_string(api.join("Program.cs")).unwrap();
    assert!(!program.contains("AccountModule") && !program.contains("AddAccount"));
    assert!(!api.join("Modules/Account/AccountSetup.cs").exists());
}
