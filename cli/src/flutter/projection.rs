//! Projects the backend contract down to the application audience before dart-dio sees it.
//!
//! Webhooks, asset endpoints, and internal operations are real backend surface but never app calls. The React
//! side filters them in the orval config; openapi-generator has no equivalent filter, so the contract itself is
//! trimmed and the components nothing reachable references are pruned with it (otherwise their models would
//! still be generated).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde_json::{Map, Value};

const HTTP_METHODS: [&str; 8] = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];
const EXCLUDED_KINDS: [&str; 3] = ["asset", "webhook", "internal"];
const EXCLUDED_TAGS: [&str; 3] = ["skies:asset", "skies:webhook", "skies:internal"];

/// Returns a copy of `document` containing only operations eligible for an application client.
pub fn project_app_client(document: &Value) -> Result<Value> {
    let Some(object) = document.as_object() else {
        bail!("the OpenAPI contract must be a JSON object")
    };
    if !object.get("paths").is_some_and(Value::is_object) {
        bail!("the OpenAPI contract must contain a paths object");
    }
    let mut projected = document.clone();
    let paths = projected["paths"].as_object_mut().expect("checked above");
    paths.retain(|_, item| {
        let Some(item) = item.as_object_mut() else {
            return true;
        };
        item.retain(|method, operation| !(is_method(method) && excluded(operation)));
        item.keys().any(|key| is_method(key))
    });
    prune_components(projected.as_object_mut().expect("checked above"));
    Ok(projected)
}

fn is_method(key: &str) -> bool {
    HTTP_METHODS.contains(&key.to_ascii_lowercase().as_str())
}

fn excluded(operation: &Value) -> bool {
    let Some(operation) = operation.as_object() else {
        return false;
    };
    if operation.get("x-skies-app-client-excluded") == Some(&Value::Bool(true)) {
        return true;
    }
    if let Some(kind) = operation.get("x-skies-endpoint-kind").and_then(Value::as_str)
        && EXCLUDED_KINDS.contains(&kind.to_ascii_lowercase().as_str())
    {
        return true;
    }
    operation.get("tags").and_then(Value::as_array).is_some_and(|tags| {
        tags.iter()
            .any(|tag| tag.as_str().is_some_and(|t| EXCLUDED_TAGS.contains(&t)))
    })
}

/// Keeps only the components reachable from the rest of the document, following `$ref`s transitively.
/// Security schemes are kept whole because they are referenced by name, not by `$ref`.
fn prune_components(document: &mut Map<String, Value>) {
    let Some(Value::Object(components)) = document.remove("components") else {
        return;
    };

    let mut reachable: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut pending: Vec<&Value> = document.values().collect();
    while let Some(node) = pending.pop() {
        match node {
            Value::String(reference) => {
                if let Some((section, name)) = parse_reference(reference) {
                    let names = reachable.entry(section.clone()).or_default();
                    if names.insert(name.clone())
                        && let Some(component) = components.get(&section).and_then(|s| s.get(&name))
                    {
                        pending.push(component);
                    }
                }
            }
            Value::Array(items) => pending.extend(items),
            Value::Object(map) => pending.extend(map.values()),
            _ => {}
        }
    }

    let mut projected = Map::new();
    for (section, values) in components {
        let Value::Object(values) = values else {
            continue;
        };
        let kept: Map<String, Value> = values
            .into_iter()
            .filter(|(name, _)| {
                section == "securitySchemes" || reachable.get(&section).is_some_and(|names| names.contains(name))
            })
            .collect();
        if !kept.is_empty() {
            projected.insert(section, Value::Object(kept));
        }
    }
    if !projected.is_empty() {
        document.insert("components".into(), Value::Object(projected));
    }
}

/// `#/components/schemas/Foo~1Bar` → `("schemas", "Foo/Bar")`, per the JSON Pointer escaping rules.
fn parse_reference(reference: &str) -> Option<(String, String)> {
    let rest = reference.strip_prefix("#/components/")?;
    let mut segments = rest.split('/').map(|s| s.replace("~1", "/").replace("~0", "~"));
    Some((segments.next()?, segments.next()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projects_only_application_operations_without_mutating_the_source() {
        let contract = json!({
            "openapi": "3.1.1",
            "paths": {
                "/wallets": { "get": { "operationId": "ListWallets", "tags": ["Wallets"] } },
                "/asset": { "get": {
                    "operationId": "Asset",
                    "tags": ["skies:asset"],
                    "responses": { "200": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/Asset" } } } } }
                } },
                "/webhook": { "post": { "operationId": "Webhook", "x-skies-endpoint-kind": "webhook" } },
                "/internal": { "get": { "operationId": "Internal", "x-skies-app-client-excluded": true } }
            },
            "components": { "schemas": { "Asset": { "type": "object" } } }
        });

        let projected = project_app_client(&contract).unwrap();

        let paths: Vec<&String> = projected["paths"].as_object().unwrap().keys().collect();
        assert_eq!(paths, ["/wallets"]);
        assert!(projected.get("components").is_none());
        assert_eq!(contract["paths"].as_object().unwrap().len(), 4);
        assert!(contract["components"]["schemas"]["Asset"].is_object());
    }

    #[test]
    fn keeps_transitively_referenced_components_and_security_schemes() {
        let contract = json!({
            "paths": { "/w": { "get": { "responses": { "200": { "$ref": "#/components/responses/Ok" } } } } },
            "components": {
                "responses": { "Ok": { "schema": { "$ref": "#/components/schemas/Wallet" } } },
                "schemas": {
                    "Wallet": { "properties": { "owner": { "$ref": "#/components/schemas/Owner" } } },
                    "Owner": { "type": "object" },
                    "Orphan": { "type": "object" }
                },
                "securitySchemes": { "Bearer": { "type": "http" } }
            }
        });

        let projected = project_app_client(&contract).unwrap();

        let schemas: Vec<&String> = projected["components"]["schemas"].as_object().unwrap().keys().collect();
        assert_eq!(schemas, ["Owner", "Wallet"]);
        assert!(projected["components"]["securitySchemes"]["Bearer"].is_object());
    }

    #[test]
    fn rejects_a_document_without_paths() {
        assert!(project_app_client(&json!({ "openapi": "3.1.0" })).is_err());
        assert!(project_app_client(&json!([])).is_err());
    }
}
