//! How an edit form opens on the record it edits.
//!
//! An `Update<Entity>` form whose contract also has the `Lookup<Entity>` slice (what `g crud` generates beside it)
//! reads the record through that slice's hook and fills its inputs from it, so the user edits what is stored instead
//! of retyping every field. The lookup is called with the form's own target (the path parameters both slices share),
//! and each input takes the record property of the same name. When the pairing is not that plain (no lookup, a
//! lookup parameter the target does not carry, a response without the inputs' properties) the form opens empty, as
//! any other form does.

use serde::Serialize;

use super::form_fields::Field;
use super::openapi::{self, Document, Node};

/// What the ViewModel template needs to fill the form from the lookup.
#[derive(Debug, PartialEq, Serialize)]
pub struct Prefill {
    /// The lookup slice whose `use<hook>` query reads the record (`LookupProduct`).
    pub hook: String,
    /// The query hook's arguments, from the form's target (`target.id`).
    pub args: String,
    /// The expression that reaches the record in the query's data (`lookup.data?.product`).
    pub record: String,
    /// Every input with the string the form holds for it, read off `record`.
    pub values: Vec<PrefillValue>,
    /// The sent-as-given fields (not typed, not in the path) the record carries, such as its concurrency `version`:
    /// a save sends the record's current value, so a second save after a first one does not replay a stale version.
    pub carried: Vec<String>,
}

/// One input's starting value.
#[derive(Debug, PartialEq, Serialize)]
pub struct PrefillValue {
    pub name: String,
    pub value: String,
}

/// The prefill for the form `form` (a slice name) over `fields`, when the contract pairs it with a lookup.
pub fn for_form(doc: &Document, form: &str, fields: &[Field]) -> Option<Prefill> {
    let entity = form.strip_prefix("Update")?;
    let hook = format!("Lookup{entity}");
    let lookup = doc.operation(&hook)?;
    if !lookup.parameters("query").is_empty() {
        return None;
    }
    let mut args = Vec::new();
    for (name, _) in lookup.parameters("path") {
        let given = fields.iter().any(|f| f.context && f.name == name);
        if !given {
            return None;
        }
        args.push(format!("target.{name}"));
    }
    let inputs: Vec<&Field> = fields.iter().filter(|f| !f.context).collect();
    let covers = |schema: &Node| -> Option<Vec<String>> {
        let props: Vec<String> = openapi::properties(doc.resolve(schema))
            .into_iter()
            .map(|(name, _, _)| name)
            .collect();
        inputs.iter().any(|input| props.contains(&input.name)).then_some(props)
    };
    let response = lookup.response()?;
    let (record, props) = match covers(response) {
        Some(props) => ("lookup.data".to_string(), props),
        None => openapi::properties(response)
            .into_iter()
            .find_map(|(name, schema, _)| Some((format!("lookup.data?.{name}"), covers(schema)?)))?,
    };
    let values = inputs
        .iter()
        .map(|input| PrefillValue {
            name: input.name.clone(),
            value: if props.contains(&input.name) {
                format!("record.{0} == null ? \"\" : String(record.{0})", input.name)
            } else {
                "\"\"".to_string()
            },
        })
        .collect();
    let carried = fields
        .iter()
        .filter(|f| f.context && f.location != "path" && props.contains(&f.name))
        .map(|f| f.name.clone())
        .collect();
    Some(Prefill {
        hook,
        args: args.join(", "),
        record,
        values,
        carried,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::form_fields;

    fn doc(lookup_params: serde_json::Value, response: serde_json::Value) -> Document {
        let doc = serde_json::json!({
            "paths": { "/catalog/products/{id}": {
                "put": { "operationId": "UpdateProduct" },
                "get": {
                    "operationId": "LookupProduct",
                    "parameters": lookup_params,
                    "responses": { "200": { "content": { "application/json": { "schema": response } } } } } } },
            "components": { "schemas": {
                "LookupProductOutput": { "type": "object", "properties": {
                    "product": { "$ref": "#/components/schemas/ProductView" } } },
                "ProductView": { "type": "object", "properties": {
                    "id": { "type": "string" }, "name": { "type": "string" }, "price": { "type": "number" } } }
            } }
        });
        Document::parse(&doc.to_string()).unwrap()
    }

    fn id() -> serde_json::Value {
        serde_json::json!([{ "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } }])
    }

    fn fields() -> Vec<Field> {
        let doc = Document::parse(
            &serde_json::json!({
                "paths": { "/p/{id}": { "put": {
                    "operationId": "UpdateProduct",
                    "parameters": id(),
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object",
                        "properties": { "name": { "type": "string" }, "price": { "type": "number" },
                            "note": { "type": "string" }, "version": { "type": "string", "format": "uuid" } } } } } }
                } } }
            })
            .to_string(),
        )
        .unwrap();
        let op = doc.operation("UpdateProduct").unwrap();
        form_fields::from_operation(&doc, &op, "UpdateProduct").unwrap()
    }

    #[test]
    fn an_update_form_reads_its_record_through_the_lookup_slice() {
        let doc = doc(
            id(),
            serde_json::json!({ "$ref": "#/components/schemas/LookupProductOutput" }),
        );
        let prefill = for_form(&doc, "UpdateProduct", &fields()).unwrap();
        assert_eq!(prefill.hook, "LookupProduct");
        assert_eq!(prefill.args, "target.id");
        assert_eq!(prefill.record, "lookup.data?.product");
        let mut values: Vec<(&str, &str)> = prefill
            .values
            .iter()
            .map(|v| (v.name.as_str(), v.value.as_str()))
            .collect();
        values.sort();
        assert_eq!(
            values,
            [
                ("name", "record.name == null ? \"\" : String(record.name)"),
                ("note", "\"\""),
                ("price", "record.price == null ? \"\" : String(record.price)"),
            ],
            "every input, the ones the record lacks empty; the version stays the target's"
        );
    }

    #[test]
    fn a_record_answered_bare_is_read_from_the_data_itself() {
        let doc = doc(id(), serde_json::json!({ "$ref": "#/components/schemas/ProductView" }));
        assert_eq!(
            for_form(&doc, "UpdateProduct", &fields()).unwrap().record,
            "lookup.data"
        );
    }

    #[test]
    fn no_prefill_without_a_plain_pairing() {
        let output = serde_json::json!({ "$ref": "#/components/schemas/LookupProductOutput" });
        assert!(for_form(&doc(id(), output.clone()), "CreateProduct", &fields()).is_none());
        let other = serde_json::json!([{ "name": "slug", "in": "path", "schema": { "type": "string" } }]);
        assert!(for_form(&doc(other, output.clone()), "UpdateProduct", &fields()).is_none());
        let unrelated = serde_json::json!({ "type": "object", "properties": { "total": { "type": "number" } } });
        assert!(for_form(&doc(id(), unrelated), "UpdateProduct", &fields()).is_none());
    }
}
