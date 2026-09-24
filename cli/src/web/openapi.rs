//! Just enough OpenAPI reading for the feature scaffold: an operation by its id (the slice name), its path
//! parameters and JSON body, and the shape of its 200 response.
//!
//! The backend's build writes the contract; `skies g client` turns it into hooks and types with orval. The feature
//! scaffold reads the same document so the screen it writes binds to the fields and names orval will generate,
//! instead of a placeholder that only typechecks against the `g slice` stub. The document is read into an ordered
//! tree ([`Node`]) because a form lists its inputs in the order the slice's `Input` declares them.

use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A JSON value whose objects keep their keys in document order.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

impl Node {
    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Object(members) => members.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Node]> {
        match self {
            Node::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Node)]> {
        match self {
            Node::Object(members) => Some(members),
            _ => None,
        }
    }

    /// The value at a path of object keys.
    fn at(&self, path: &[&str]) -> Option<&Node> {
        path.iter().try_fold(self, |node, key| node.get(key))
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Node, D::Error> {
        struct NodeVisitor;
        impl<'de> Visitor<'de> for NodeVisitor {
            type Value = Node;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_unit<E>(self) -> std::result::Result<Node, E> {
                Ok(Node::Null)
            }
            fn visit_bool<E>(self, value: bool) -> std::result::Result<Node, E> {
                Ok(Node::Bool(value))
            }
            fn visit_i64<E>(self, value: i64) -> std::result::Result<Node, E> {
                Ok(Node::Number(value as f64))
            }
            fn visit_u64<E>(self, value: u64) -> std::result::Result<Node, E> {
                Ok(Node::Number(value as f64))
            }
            fn visit_f64<E>(self, value: f64) -> std::result::Result<Node, E> {
                Ok(Node::Number(value))
            }
            fn visit_str<E>(self, value: &str) -> std::result::Result<Node, E> {
                Ok(Node::String(value.to_string()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Node, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Node::Array(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Node, A::Error> {
                let mut members = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Node>()? {
                    members.push((key, value));
                }
                Ok(Node::Object(members))
            }
        }
        deserializer.deserialize_any(NodeVisitor)
    }
}

/// A parsed OpenAPI document.
pub struct Document {
    root: Node,
}

/// One operation of the document.
pub struct Operation<'a> {
    doc: &'a Document,
    value: &'a Node,
}

impl Document {
    pub fn load(path: &Path) -> Result<Document> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Document::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Document> {
        Ok(Document {
            root: serde_json::from_str(text)?,
        })
    }

    /// The operation whose `operationId` is `id` (a slice's `.WithName(nameof(Slice))`).
    pub fn operation(&self, id: &str) -> Option<Operation<'_>> {
        self.operations()
            .find(|op| op.get("operationId").and_then(Node::as_str) == Some(id))
            .map(|value| Operation { doc: self, value })
    }

    fn operations(&self) -> impl Iterator<Item = &Node> {
        self.root
            .get("paths")
            .and_then(Node::as_object)
            .unwrap_or_default()
            .iter()
            .filter_map(|(_, item)| item.as_object())
            .flat_map(|methods| methods.iter().map(|(_, op)| op))
    }

    /// Every `operationId` in the document, sorted: the names a "did you mean" message can offer.
    pub fn operation_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .operations()
            .filter_map(|op| op.get("operationId")?.as_str().map(str::to_string))
            .collect();
        ids.sort();
        ids
    }

    /// Follows a `$ref` (and a nullable `oneOf`/`anyOf`/`allOf` wrapper around one) to the schema it names.
    pub fn resolve<'a>(&'a self, schema: &'a Node) -> &'a Node {
        if let Some(reference) = schema.get("$ref").and_then(Node::as_str) {
            let name = reference.rsplit('/').next().unwrap_or_default();
            return self
                .root
                .at(&["components", "schemas", name])
                .map_or(schema, |target| self.resolve(target));
        }
        for key in ["oneOf", "anyOf", "allOf"] {
            if let Some(variants) = schema.get(key).and_then(Node::as_array) {
                let real: Vec<&Node> = variants.iter().filter(|v| !is_null(v)).collect();
                if let [only] = real.as_slice() {
                    return self.resolve(only);
                }
            }
        }
        schema
    }
}

/// The component name a schema refers to (`#/components/schemas/ProductView` → `ProductView`), through a nullable
/// wrapper.
pub fn schema_name(schema: &Node) -> Option<String> {
    if let Some(reference) = schema.get("$ref").and_then(Node::as_str) {
        return reference.rsplit('/').next().map(str::to_string);
    }
    ["oneOf", "anyOf", "allOf"].iter().find_map(|key| {
        let variants = schema.get(key)?.as_array()?;
        let real: Vec<&Node> = variants.iter().filter(|v| !is_null(v)).collect();
        match real.as_slice() {
            [only] => schema_name(only),
            _ => None,
        }
    })
}

/// The schema's non-null `type` (`["string", "null"]` → `string`).
pub fn type_of(schema: &Node) -> Option<&str> {
    match schema.get("type")? {
        Node::String(kind) => Some(kind.as_str()),
        Node::Array(kinds) => kinds.iter().filter_map(Node::as_str).find(|kind| *kind != "null"),
        _ => None,
    }
}

/// Whether the schema admits `null`.
pub fn is_nullable(schema: &Node) -> bool {
    schema.get("nullable") == Some(&Node::Bool(true))
        || schema
            .get("type")
            .and_then(Node::as_array)
            .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("null")))
        || ["oneOf", "anyOf"].iter().any(|key| {
            schema
                .get(key)
                .and_then(Node::as_array)
                .is_some_and(|variants| variants.iter().any(is_null))
        })
}

fn is_null(schema: &Node) -> bool {
    schema.get("type").and_then(Node::as_str) == Some("null")
}

/// An object schema's properties in document order, each with whether it is required.
pub fn properties(schema: &Node) -> Vec<(String, &Node, bool)> {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Node::as_array)
        .map(|names| names.iter().filter_map(Node::as_str).collect())
        .unwrap_or_default();
    schema
        .get("properties")
        .and_then(Node::as_object)
        .unwrap_or_default()
        .iter()
        .map(|(name, value)| (name.clone(), value, required.contains(&name.as_str())))
        .collect()
}

impl<'a> Operation<'a> {
    /// The operation's parameters at `location` (`path`, `query`), each with its schema.
    pub fn parameters(&self, location: &str) -> Vec<(String, &'a Node)> {
        self.value
            .get("parameters")
            .and_then(Node::as_array)
            .unwrap_or_default()
            .iter()
            .filter(|p| p.get("in").and_then(Node::as_str) == Some(location))
            .filter_map(|p| Some((p.get("name")?.as_str()?.to_string(), p.get("schema")?)))
            .collect()
    }

    /// The JSON request body's schema, resolved, when the operation takes one.
    pub fn body(&self) -> Option<&'a Node> {
        let schema = self
            .value
            .at(&["requestBody", "content", "application/json", "schema"])?;
        Some(self.doc.resolve(schema))
    }

    /// The 200 response's JSON schema, resolved.
    pub fn response(&self) -> Option<&'a Node> {
        let schema = self
            .value
            .at(&["responses", "200", "content", "application/json", "schema"])?;
        Some(self.doc.resolve(schema))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        Document::parse(
            r##"{
            "paths": {
                "/catalog/product/{id}": { "put": {
                    "operationId": "UpdateProduct",
                    "parameters": [{ "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } }],
                    "requestBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/UpdateProductInput" } } } }
                } },
                "/catalog/product": { "get": {
                    "operationId": "ListProduct",
                    "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ListProductOutput" } } } } }
                } }
            },
            "components": { "schemas": {
                "UpdateProductInput": { "type": "object", "required": ["title"], "properties": {
                    "title": { "type": "string" }, "note": { "type": ["string", "null"] } } },
                "ListProductOutput": { "type": "object", "properties": {
                    "products": { "$ref": "#/components/schemas/PageOfProductView" } } },
                "PageOfProductView": { "type": "object", "properties": {
                    "items": { "type": "array", "items": { "$ref": "#/components/schemas/ProductView" } } } },
                "ProductView": { "type": "object", "properties": { "id": { "type": "string" } } }
            } }
        }"##,
        )
        .unwrap()
    }

    #[test]
    fn finds_an_operation_its_parameters_body_and_response() {
        let doc = doc();
        let update = doc.operation("UpdateProduct").unwrap();
        assert_eq!(update.parameters("path")[0].0, "id");
        let body = properties(update.body().unwrap());
        assert_eq!(
            body.iter().map(|p| (p.0.as_str(), p.2)).collect::<Vec<_>>(),
            [("title", true), ("note", false)],
            "document order, not alphabetical"
        );
        assert!(is_nullable(body[1].1) && type_of(body[1].1) == Some("string"));

        let list = doc.operation("ListProduct").unwrap();
        let page = doc.resolve(properties(list.response().unwrap())[0].1);
        let row = page.at(&["properties", "items", "items"]).unwrap();
        assert_eq!(schema_name(row).as_deref(), Some("ProductView"));
        assert!(doc.operation("Missing").is_none());
        assert_eq!(doc.operation_ids(), ["ListProduct", "UpdateProduct"]);
    }
}
