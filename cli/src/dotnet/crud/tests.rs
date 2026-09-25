use super::*;

#[test]
fn splits_scalar_fields_from_complex_and_system_ones() {
    let (scalars, complex) = parse_fields(
        "public Guid Id { get; private set; }\npublic string Name { get; private set; }\n\
         public int? Age { get; set; }\npublic Email Contact { get; private set; }\n\
         public Guid OrgId { get; private set; }\npublic Guid TenantId { get; private set; }\n\
         public byte[]? RowVersion { get; private set; }\npublic string Slug => Name;",
    );
    let names: Vec<_> = scalars.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
    assert_eq!(names, ["string Name", "int? Age"]);
    assert_eq!(complex.len(), 1);
    assert_eq!(complex[0].ty, "Email");
}

#[test]
fn the_view_keeps_identity_and_stamps_but_never_tenancy_or_the_token() {
    let view = view_fields(
        "public Guid Id { get; private set; }\npublic Guid OrgId { get; private set; }\n\
         public string Name { get; private set; }\npublic Email Contact { get; private set; }\n\
         public DateTime CreatedAt { get; private set; }\npublic byte[]? RowVersion { get; private set; }",
    );
    let names: Vec<_> = view.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
    assert_eq!(names, ["Guid Id", "string Name", "DateTime CreatedAt"]);
}

#[test]
fn tenancy_is_read_off_the_entity_declaration() {
    assert!(is_tenant_scoped(
        "[Entity]\npublic class Product : ITenantScoped\n{",
        "Product"
    ));
    assert!(!is_tenant_scoped(
        "using Acme.Tenancy; // ITenantScoped lives here\n[Entity]\npublic class Product\n{",
        "Product"
    ));
}

#[test]
fn the_marker_in_a_comment_never_makes_an_entity_tenant_scoped() {
    for source in [
        "[Entity]\npublic class Product // not ITenantScoped: every org shares it\n{",
        "[Entity]\npublic class Product : IAuditable /* ITenantScoped later */\n{",
        "/// <summary>Unlike class Product : ITenantScoped in v1, app-wide.</summary>\n[Entity]\npublic class Product\n{",
        "[Entity]\npublic class Product\n{\n    // class Product : ITenantScoped\n}",
        "public class Other : ITenantScoped { }\n[Entity]\npublic class Product\n{",
    ] {
        assert!(!is_tenant_scoped(source, "Product"), "{source}");
    }
}

#[test]
fn the_marker_is_read_from_the_base_list_in_any_position_or_spelling() {
    for source in [
        "public class Product : Entity<Guid, string>, ITenantScoped\n{",
        "public sealed class Product : IAuditable,\n    Skies.Framework.EntityFrameworkCore.ITenantScoped // scoped\n{",
        "public class Product<T> : global::Skies.Framework.EntityFrameworkCore.ITenantScoped where T : new()\n{",
        "public record Product(Guid Id) : ITenantScoped;",
    ] {
        assert!(is_tenant_scoped(source, "Product"), "{source}");
    }
    assert!(!is_tenant_scoped(
        "public class ProductLine : ITenantScoped\n{",
        "Product"
    ));
}

const APP_DB: &str = concat!(
    "using Microsoft.EntityFrameworkCore;\n\nnamespace Acme.Api;\n\n",
    "public class AppDb(DbContextOptions<AppDb> options) : DbContext(options)\n{\n",
    "    public DbSet<Other> Others => Set<Other>();\n}\n",
);

const ENTITY: &str = concat!(
    "namespace Acme.Api.Modules.Catalog;\n\n[Entity]\npublic class Product\n{\n",
    "    public Guid Id { get; private set; }\n\n    public string Name { get; private set; } = \"\";\n\n",
    "    private Product() { }\n\n",
    "    /// <summary>Open a new Product with the given identity.</summary>\n",
    "    public static Result<Product> Open(Guid id) =>\n        new Product { Id = id }.EnsureValid();\n\n",
    "    private Result<Product> EnsureValid()\n    {\n        return this;\n    }\n}\n",
);

fn project(entity: &str, app_db: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Acme.Api.csproj"),
        "<Project>\n  <ItemGroup>\n  </ItemGroup>\n</Project>\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("Modules/Catalog")).unwrap();
    std::fs::write(
        dir.path().join("Modules/Catalog/CatalogModule.cs"),
        "public static class CatalogModule\n{\n    public static void Map(IEndpointRouteBuilder app)\n    {\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("Modules/Catalog/Product.cs"), entity).unwrap();
    if let Some(app_db) = app_db {
        std::fs::write(dir.path().join("AppDb.cs"), app_db).unwrap();
    }
    dir
}

fn read(dir: &tempfile::TempDir, path: &str) -> String {
    std::fs::read_to_string(dir.path().join(path)).unwrap()
}

#[test]
fn an_unmarked_row_is_refused_rather_than_left_unchecked() {
    let dir = project(
        "public class Product { public string Name { get; set; } }",
        Some(APP_DB),
    );
    assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
    assert!(!dir.path().join("Modules/Catalog/Slices").exists());
}

#[test]
fn an_entity_with_no_fields_is_refused() {
    let dir = project(
        "[Entity]\npublic class Product { public Guid Id { get; private set; } }",
        Some(APP_DB),
    );
    assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
}

#[test]
fn an_app_without_app_db_is_refused_before_writing_anything() {
    let dir = project(ENTITY, None);
    assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 1);
    assert!(!dir.path().join("Modules/Catalog/Slices").exists());
    assert_eq!(read(&dir, "Modules/Catalog/Product.cs"), ENTITY);
}

#[test]
fn an_app_wide_entity_gets_registered_projected_slices_under_the_group() {
    let dir = project(ENTITY, Some(APP_DB));

    assert_eq!(generate(dir.path(), "Catalog", "Product").unwrap(), 0);

    let app_db = read(&dir, "AppDb.cs");
    assert!(app_db.contains("    public DbSet<Product> Products => Set<Product>();\n}"));
    assert!(app_db.contains("using Acme.Api.Modules.Catalog;\n"));

    let view = read(&dir, "Modules/Catalog/ProductView.cs");
    assert!(view.contains("public record ProductView(Guid Id, string Name, Guid Version)"));
    assert!(view.contains("public static ProductView From(Product e) => new(e.Id, e.Name, e.Version);"));
    assert!(view.starts_with("namespace Acme.Api.Modules.Catalog;\n\n/// <summary>A Product as the API returns it:"));

    let list = read(&dir, "Modules/Catalog/Slices/ListProducts.cs");
    assert!(list.contains("public static class ListProducts"));
    assert!(list.contains("public record Output(Page<ProductView> Products);"));
    assert!(list.contains("return new Output(page.Select(ProductView.From));"));
    assert!(list.contains("app.MapGet(\"/products\","));
    let lookup = read(&dir, "Modules/Catalog/Slices/LookupProduct.cs");
    assert!(lookup.contains("public record Output(ProductView Product);"));
    assert!(lookup.contains("CatalogErrorCodes.ProductNotFound"));

    let update = read(&dir, "Modules/Catalog/Slices/UpdateProduct.cs");
    assert!(update.contains("public record Changes(string Name, Guid Version);"));
    assert!(update.contains("app.MapPut(\"/products/{id:guid}\","));
    assert!(update.contains("(await Handle(new Input(id, changes.Name, changes.Version), db, ct)).ToHttp())"));
    assert!(update.contains("db.Entry(item).Property(e => e.Version).OriginalValue = input.Version;"));
    assert!(update.contains("catch (DbUpdateConcurrencyException)"));
    let delete = read(&dir, "Modules/Catalog/Slices/DeleteProduct.cs");
    assert!(delete.contains("app.MapDelete(\"/products/{id:guid}\", async (Guid id, Guid version,"));
    let codes = read(&dir, "Modules/Catalog/CatalogErrorCodes.cs");
    assert!(codes.contains("ProductNotFound = \"catalog.product_not_found\""));
    assert!(codes.contains("ProductChanged = \"catalog.product_changed\""));

    for slice in [
        "ListProducts",
        "LookupProduct",
        "CreateProduct",
        "UpdateProduct",
        "DeleteProduct",
    ] {
        let source = read(&dir, &format!("Modules/Catalog/Slices/{slice}.cs"));
        assert!(
            !source.contains("RequireAuthorization"),
            "{slice} restates the group's posture"
        );
        assert!(
            !source.contains("org"),
            "{slice} speaks of tenancy for an app-wide entity"
        );
        assert!(!source.contains("SKY"), "{slice} cites a rule");
    }
    // No auth blueprint here, so nothing names an admin: the app-wide writes are mapped closed, never open.
    let module = read(&dir, "Modules/Catalog/CatalogModule.cs");
    assert!(module.contains(concat!(
        "        var catalog = app.MapGroup(\"/catalog\").RequireAuthorization();\n",
        "        ListProducts.Map(catalog);\n",
        "        LookupProduct.Map(catalog);\n",
    )));
    assert!(module.contains(concat!(
        "        var catalogAdmin = app.MapGroup(\"/catalog\").RequireAuthorization(policy => policy.RequireAssertion(_ => false));\n",
        "        CreateProduct.Map(catalogAdmin);\n",
        "        UpdateProduct.Map(catalogAdmin);\n",
        "        DeleteProduct.Map(catalogAdmin);\n",
    )));

    assert_eq!(
        generate(dir.path(), "Catalog", "Product").unwrap(),
        0,
        "a rerun is a no-op"
    );
    assert_eq!(read(&dir, "AppDb.cs").matches("DbSet<Product>").count(), 1);
}

#[test]
fn a_tenant_scoped_entity_says_so_and_its_article_is_right() {
    let invoice = ENTITY
        .replace("Product", "Invoice")
        .replace("public class Invoice\n", "public class Invoice : ITenantScoped\n")
        .replace(
            "    public Guid Id { get; private set; }\n",
            "    public Guid Id { get; private set; }\n\n    public Guid OrgId { get; private set; }\n",
        );
    let dir = project("", Some(APP_DB));
    std::fs::write(dir.path().join("Modules/Catalog/Invoice.cs"), &invoice).unwrap();

    assert_eq!(generate(dir.path(), "Catalog", "Invoice").unwrap(), 0);

    let create = read(&dir, "Modules/Catalog/Slices/CreateInvoice.cs");
    assert!(create.contains("/// <summary>Create an Invoice within the caller's org."));
    assert!(create.contains("The org is stamped by the DbContext"));
    let view = read(&dir, "Modules/Catalog/InvoiceView.cs");
    assert!(view.contains("/// <summary>An Invoice as the API returns it"));
    assert!(view.contains("public record InvoiceView(Guid Id, string Name, Guid Version)"));
    assert!(read(&dir, "AppDb.cs").contains("public DbSet<Invoice> Invoices => Set<Invoice>();"));
    // Tenant-scoped rows are the caller's org's own, so every slice stays under the module group.
    let module = read(&dir, "Modules/Catalog/CatalogModule.cs");
    assert!(module.contains("        DeleteInvoice.Map(catalog);\n"));
    assert!(!module.contains("catalogAdmin"));
}
