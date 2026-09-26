use super::*;

fn locales(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn rendered(name: &str) -> Vec<(String, String)> {
    let names = FeatureNames::derive(name).unwrap();
    let shape = Shape::List {
        slice: list_slice(&names, None),
        rows: None,
    };
    render_feature(&names, &shape, "shop", &locales(&["ptBR", "esES", "enUS"])).unwrap()
}

fn form(name: &str, spec: &str) -> Vec<(String, String)> {
    let names = FeatureNames::derive(name).unwrap();
    let fields = form_fields::parse(spec).unwrap();
    let shape = Shape::Form {
        variables: variables(&fields, true),
        fields,
        prefill: None,
    };
    render_feature(&names, &shape, "shop", &locales(&["ptBR", "esES", "enUS"])).unwrap()
}

/// The keys of one locale block of a rendered i18n module.
fn keys(i18n: &str, locale: &str) -> Vec<String> {
    let block = i18n.split(&format!("export const {locale} = {{")).nth(1).unwrap();
    let block = &block[..block.find("} as const").unwrap()];
    block
        .lines()
        .filter_map(|line| line.trim().split(':').next().map(str::to_string))
        .filter(|key| !key.is_empty())
        .collect()
}

/// A package inside a Skies app whose backend contract holds `paths` and `schemas`.
fn package_with_contract(paths: serde_json::Value, schemas: serde_json::Value) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Skies.toml"),
        "[workspace]\nname = \"s\"\n[products.app]\nbackend = \"api/Shop.Api\"\nfrontend = \"web\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("api/Shop.Api/contract")).unwrap();
    let doc = serde_json::json!({ "paths": paths, "components": { "schemas": schemas } });
    std::fs::write(dir.path().join("api/Shop.Api/contract/Shop.Api.json"), doc.to_string()).unwrap();
    std::fs::create_dir_all(dir.path().join("web/src")).unwrap();
    let web = dir.path().join("web").canonicalize().unwrap();
    (dir, web)
}

fn list_contract(operation: &str) -> (tempfile::TempDir, PathBuf) {
    package_with_contract(
        serde_json::json!({ "/catalog/products": { "get": {
            "operationId": operation,
            "responses": { "200": { "content": { "application/json": { "schema": {
                "$ref": "#/components/schemas/ListProductsOutput" } } } } } } } }),
        serde_json::json!({
            "ListProductsOutput": { "type": "object", "properties": {
                "products": { "$ref": "#/components/schemas/PageOfProductView" } } },
            "PageOfProductView": { "type": "object", "properties": {
                "items": { "type": "array", "items": { "$ref": "#/components/schemas/ProductView" } } } },
            "ProductView": { "type": "object", "properties": {
                "id": { "type": "string", "format": "uuid" }, "name": { "type": "string" } } }
        }),
    )
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

#[test]
fn derives_names_from_a_plural_feature_name() {
    let names = FeatureNames::derive("user-profiles").unwrap();
    assert_eq!(names.plural, "UserProfiles");
    assert_eq!(names.collection, "userProfiles");
    assert_eq!(names.entity, "UserProfile");
    assert_eq!(names.lower, "user-profiles");
    assert_eq!(FeatureNames::derive("CreateProduct").unwrap().lower, "create-product");
}

#[test]
fn the_list_slice_is_the_plural_crud_name() {
    let names = FeatureNames::derive("Products").unwrap();
    assert_eq!(list_slice(&names, None), "ListProducts");
    let doc = Document::parse(r#"{ "paths": { "/p": { "get": { "operationId": "ListProduct" } } } }"#).unwrap();
    assert_eq!(
        list_slice(&names, Some(&doc)),
        "ListProduct",
        "a contract from crud before the plural naming"
    );
}

#[test]
fn emits_the_three_files_of_the_unit_and_no_tests() {
    let files: Vec<String> = rendered("bookings").into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        files,
        ["Bookings.viewModel.ts", "Bookings.view.tsx", "bookings.i18n.ts"]
    );
}

#[test]
fn wires_the_spine_without_proof_ceremony() {
    let files = rendered("bookings");
    let (view_model, view, i18n) = (&files[0].1, &files[1].1, &files[2].1);

    assert!(view_model.contains("AsyncState<Booking[]>"));
    assert!(view_model.contains("import { useListBookings } from \"@/client.gen/shop\";"));
    assert!(view_model.contains("i18n.t(\"bookings:error\")"));
    assert!(
        view_model.contains("data: query.data?.bookings?.items,"),
        "the List slice returns a page"
    );
    assert!(view.contains("<Resource"));
    assert!(view.contains("{(bookings) => <BookingsList bookings={bookings} />}"));
    assert!(view.contains("{bookings.map((item) => ("));
    for locale in ["ptBR", "esES", "enUS"] {
        assert!(i18n.contains(&format!("export const {locale}")));
    }
    for (_, contents) in &files {
        for ceremony in ["@verify", "@avp", "@e2e", "defineVerification", "{{", "{%"] {
            assert!(!contents.contains(ceremony), "{ceremony} leaked into the scaffold");
        }
    }
}

#[test]
fn a_list_binds_to_the_contracts_page_and_row_type() {
    let (_dir, web) = list_contract("ListProducts");

    scaffold(&web, "Products", FeatureKind::List, None).unwrap();

    let view_model = read(&web, "src/products/Products.viewModel.ts");
    assert!(view_model.contains("import type { ProductView } from \"@/client.gen/model\";"));
    assert!(view_model.contains("import { useListProducts } from \"@/client.gen/shop\";"));
    assert!(view_model.contains("export type Product = ProductView;"));
    assert!(view_model.contains("data: query.data?.products?.items,"));
    assert!(read(&web, "src/products/Products.view.tsx").contains("<Text key={item.id}>{item.name}</Text>"));

    let (_old, legacy) = list_contract("ListProduct");
    scaffold(&legacy, "Products", FeatureKind::List, None).unwrap();
    assert!(read(&legacy, "src/products/Products.viewModel.ts").contains("import { useListProduct } from"));
}

#[test]
fn a_form_reads_the_commands_inputs_from_the_contract() {
    let (_dir, web) = package_with_contract(
        serde_json::json!({ "/catalog/products": { "post": {
            "operationId": "CreateProduct",
            "requestBody": { "content": { "application/json": { "schema": {
                "$ref": "#/components/schemas/CreateProductInput" } } } } } } }),
        serde_json::json!({ "CreateProductInput": { "type": "object", "required": ["name", "price"], "properties": {
            "name": { "type": "string" }, "price": { "type": "number", "format": "double" } } } }),
    );

    scaffold(&web, "CreateProduct", FeatureKind::Form, None).unwrap();

    let dir = web.join("src/create-product");
    let view_model = read(&dir, "CreateProduct.viewModel.ts");
    assert!(view_model.contains("import { useCreateProduct } from \"@/client.gen/shop\";"));
    assert!(view_model.contains("  name: string;\n  price: string;\n}"));
    assert!(view_model.contains("name: z.string().trim().min(1, i18n.t(\"create-product:errors.name\")),"));
    assert!(view_model.contains("mutation.mutate({ data: { name: values.name, price: Number(values.price) } })"));
    assert!(view_model.contains("order: [\"name\", \"price\"]"));
    assert!(view_model.contains("i18n.t(\"create-product:errors.submit\")"));
    let view = read(&dir, "CreateProduct.view.tsx");
    assert!(view.contains("useTranslation(\"create-product\")"));
    assert!(view.contains("onChange={field.onChange}") && view.contains("onClick={submit}"));
    assert!(view.contains("name=\"price\"") && view.contains("kind=\"number\""));
    let i18n = read(&dir, "create-product.i18n.ts");
    assert!(i18n.contains("  title: \"Create product\",\n") && i18n.contains("\"fields.price.label\": \"Price\","));

    let error = format!(
        "{:#}",
        scaffold(&web, "PublishProduct", FeatureKind::Form, None).unwrap_err()
    );
    assert!(
        error.contains("no `PublishProduct` operation") && error.contains("CreateProduct"),
        "{error}"
    );
    assert!(!web.join("src/publish-product").exists());
}

#[test]
fn a_form_without_a_contract_takes_fields_or_stops() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();

    let error = format!(
        "{:#}",
        scaffold(dir.path(), "Transfer", FeatureKind::Form, None).unwrap_err()
    );
    assert!(
        error.contains("--fields") && error.contains("nothing was written"),
        "{error}"
    );
    assert!(!dir.path().join("src/transfer").exists());

    scaffold(
        dir.path(),
        "Transfer",
        FeatureKind::Form,
        Some("toWalletId:uuid,amount:number"),
    )
    .unwrap();
    let view_model = read(dir.path(), "src/transfer/Transfer.viewModel.ts");
    assert!(
        view_model
            .contains("mutation.mutate({ data: { toWalletId: values.toWalletId, amount: Number(values.amount) } })")
    );
    assert!(scaffold(dir.path(), "Other", FeatureKind::List, Some("a:string")).is_err());
}

#[test]
fn a_form_is_the_command_recipe() {
    let files = form("transfer", "walletId:uuid,amount:number");
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["Transfer.viewModel.ts", "Transfer.view.tsx", "transfer.i18n.ts"]
    );
    let (view_model, view, i18n) = (&files[0].1, &files[1].1, &files[2].1);

    assert!(view_model.contains("import { useTransfer } from \"@/client.gen/shop\";"));
    assert!(view_model.contains("const form = useForm<TransferForm>({"));
    assert!(view_model.contains("const submit = submitOrReveal(\n    form.handleSubmit,"));
    assert!(view_model.contains("walletId: z.uuid(i18n.t(\"transfer:errors.walletId\")),"));
    assert!(view_model.contains("defaultValues: { walletId: \"\", amount: \"\" },"));
    assert!(view_model.contains("submitting: mutation.isPending,"));
    assert!(view_model.contains("submitError: mutation.isError ? i18n.t(\"transfer:errors.submit\") : null,"));
    assert!(view_model.contains("completed: mutation.isSuccess,"));
    assert!(!view_model.contains("useList") && !view_model.contains("AsyncState"));
    assert!(view.contains("render={({ field, fieldState }) => ("));
    assert!(view.contains("error={fieldState.error?.message}"));
    assert!(view.contains("<Text role=\"label\" tone=\"danger\" alert>"));
    assert!(view.contains("loading={submitting}"));
    assert!(!view.contains("EmptyState") && !view.contains("client.gen"));
    assert!(!view.contains("onChangeText") && !view.contains("onPress"));
    assert_eq!(keys(i18n, "ptBR"), keys(i18n, "enUS"));
    assert_eq!(keys(i18n, "esES"), keys(i18n, "enUS"));
    assert!(i18n.contains("  title: \"Transfer\",\n"));
    for (_, contents) in &files {
        assert!(!contents.contains("{{") && !contents.contains("{%"));
    }
}

#[test]
fn the_locale_set_is_the_packages_and_defaults_to_english() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/b")).unwrap();
    assert_eq!(app_locales(dir.path()).unwrap(), ["en"]);

    scaffold(dir.path(), "Items", FeatureKind::List, None).unwrap();
    let i18n = read(dir.path(), "src/items/items.i18n.ts");
    assert!(i18n.contains("export const en = {"));
    assert_eq!(i18n.matches("export const").count(), 1);
    assert!(i18n.ends_with("} as const;\n") && !i18n.ends_with("\n\n"));

    std::fs::remove_file(dir.path().join("src/items/items.i18n.ts")).unwrap();
    std::fs::write(
        dir.path().join("src/b/a.i18n.ts"),
        "export const frFR = {} as const;\nexport const deDE = {} as const;\n",
    )
    .unwrap();
    assert_eq!(app_locales(dir.path()).unwrap(), ["frFR", "deDE"]);

    scaffold(dir.path(), "Transfer", FeatureKind::Form, Some("amount:number")).unwrap();
    let i18n = read(dir.path(), "src/transfer/transfer.i18n.ts");
    assert_eq!(keys(&i18n, "frFR"), keys(&i18n, "deDE"));
    assert!(!i18n.contains("enUS") && !i18n.contains("ptBR"));
}

#[test]
fn scaffolds_cite_no_rule_ids() {
    for (_, contents) in rendered("bookings").into_iter().chain(form("transfer", "id:uuid")) {
        assert!(
            !contents.contains("SKY"),
            "a rule id leaked into the scaffold:\n{contents}"
        );
    }
}

#[test]
fn rejects_a_name_without_letters() {
    assert!(FeatureNames::derive("--").is_err());
    assert!(FeatureNames::derive("9lives").is_err());
}

#[test]
fn writes_under_src_and_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();

    scaffold(dir.path(), "Profile", FeatureKind::List, None).unwrap();

    assert!(dir.path().join("src/profile/Profile.viewModel.ts").is_file());
    assert!(scaffold(dir.path(), "Profile", FeatureKind::Form, Some("id:uuid")).is_err());
}

#[test]
fn the_client_module_is_the_packages_not_the_features() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Skies.toml"),
        "[workspace]\nname = \"s\"\n[products.app]\nbackend = \"api/Sample.Api\"\nfrontend = \"web\"\n",
    )
    .unwrap();
    let web = dir.path().join("web");
    std::fs::create_dir_all(web.join("src/client.gen/model")).unwrap();
    let web = web.canonicalize().unwrap();
    assert_eq!(client_module(&web), "sample", "the name g client will write");

    std::fs::write(web.join("src/client.gen/other.ts"), "").unwrap();
    std::fs::write(web.join("src/client.gen/sample.ts"), "").unwrap();
    assert_eq!(client_module(&web), "sample");

    let lone = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(lone.path().join("src/client.gen")).unwrap();
    std::fs::write(lone.path().join("src/client.gen/shop.ts"), "").unwrap();
    assert_eq!(client_module(lone.path()), "shop");
}

#[test]
fn a_form_takes_the_record_it_acts_on_as_its_target() {
    let (_dir, web) = package_with_contract(
        serde_json::json!({ "/catalog/products/{id}": {
            "put": {
                "operationId": "UpdateProduct",
                "parameters": [{ "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } }],
                "requestBody": { "content": { "application/json": { "schema": {
                    "$ref": "#/components/schemas/UpdateProductChanges" } } } } },
            "delete": {
                "operationId": "DeleteProduct",
                "parameters": [
                    { "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } },
                    { "name": "version", "in": "query", "schema": { "type": "string", "format": "uuid" } }] } } }),
        serde_json::json!({ "UpdateProductChanges": { "type": "object", "required": ["name", "version"], "properties": {
            "name": { "type": "string" }, "version": { "type": "string", "format": "uuid" } } } }),
    );

    scaffold(&web, "UpdateProduct", FeatureKind::Form, None).unwrap();
    scaffold(&web, "DeleteProduct", FeatureKind::Form, None).unwrap();

    let update = read(&web, "src/update-product/UpdateProduct.viewModel.ts");
    assert!(update.contains("export interface UpdateProductTarget {\n  id: string;\n  version: string;\n}"));
    assert!(
        update.contains("export function useUpdateProductModel(target: UpdateProductTarget): UpdateProductModel {")
    );
    assert!(update.contains("export interface UpdateProductForm {\n  name: string;\n}"));
    assert!(
        update.contains("mutation.mutate({ id: target.id, data: { name: values.name, version: target.version } })")
    );
    let view = read(&web, "src/update-product/UpdateProduct.view.tsx");
    assert!(view.contains("export function UpdateProductView({ target }: { target: UpdateProductTarget }) {"));
    assert!(view.contains("useUpdateProductModel(target);") && !view.contains("name=\"version\""));
    assert!(!read(&web, "src/update-product/update-product.i18n.ts").contains("version"));

    let delete = read(&web, "src/delete-product/DeleteProduct.viewModel.ts");
    assert!(
        delete.contains("    submit: () => mutation.mutate({ id: target.id, params: { version: target.version } }),")
    );
    assert!(!delete.contains("useForm") && !delete.contains("zod") && !delete.contains("Control<"));
    let view = read(&web, "src/delete-product/DeleteProduct.view.tsx");
    assert!(!view.contains("Controller") && !view.contains("control"));
    assert!(view.contains("import { Button, Card, Screen, Stack, Text } from \"@/ui\";"));
}

#[test]
fn an_update_form_opens_on_its_record_and_names_a_conflict() {
    let id = serde_json::json!([{ "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } }]);
    let (_dir, web) = package_with_contract(
        serde_json::json!({ "/catalog/products/{id}": {
            "put": {
                "operationId": "UpdateProduct",
                "parameters": id,
                "requestBody": { "content": { "application/json": { "schema": {
                    "$ref": "#/components/schemas/UpdateProductChanges" } } } } },
            "get": {
                "operationId": "LookupProduct",
                "parameters": id,
                "responses": { "200": { "content": { "application/json": { "schema": {
                    "$ref": "#/components/schemas/LookupProductOutput" } } } } } } } }),
        serde_json::json!({
            "UpdateProductChanges": { "type": "object", "required": ["name", "version"], "properties": {
                "name": { "type": "string" }, "version": { "type": "string", "format": "uuid" } } },
            "LookupProductOutput": { "type": "object", "properties": {
                "product": { "$ref": "#/components/schemas/ProductView" } } },
            "ProductView": { "type": "object", "properties": {
                "id": { "type": "string" }, "name": { "type": "string" }, "version": { "type": "string" } } } }),
    );

    scaffold(&web, "UpdateProduct", FeatureKind::Form, None).unwrap();

    let update = read(&web, "src/update-product/UpdateProduct.viewModel.ts");
    assert!(update.contains("import { useUpdateProduct, useLookupProduct } from \"@/client.gen/"));
    assert!(update.contains("  const lookup = useLookupProduct(target.id);\n  const record = lookup.data?.product;\n"));
    assert!(update.contains(
        "    values: record\n      ? { name: record.name == null ? \"\" : String(record.name) }\n      : undefined,\n    \
         resetOptions: { keepDirtyValues: true },\n"
    ));
    // The save sends the version the record has now, so a second save after a first one is not a stale write.
    assert!(update.contains(
        "mutation.mutate({ id: target.id, data: { name: values.name, version: record?.version ?? target.version } })"
    ));
    assert!(update.contains("?.response?.status === 409;"));
    assert!(
        update.contains("i18n.t(conflict ? \"update-product:errors.conflict\" : \"update-product:errors.submit\")")
    );
    let copy = read(&web, "src/update-product/update-product.i18n.ts");
    assert!(copy.contains("\"errors.conflict\": \"Someone else changed this after you opened it. Reload"));

    let create = form("CreateProduct", "name:string");
    assert!(!create[0].1.contains("lookup") && !create[0].1.contains("409"));
    assert!(!create[2].1.contains("errors.conflict"));
}
