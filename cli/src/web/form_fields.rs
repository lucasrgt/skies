//! The inputs a `--kind form` screen edits, and what the list screen renders: read from the backend contract, or
//! from `--fields` when there is no contract yet.
//!
//! A form field is a text box: the ViewModel's form keeps every value a string (what an input hands back), the zod
//! schema restates the contract's requiredness and format, and the submit converts at the boundary (`Number(...)`).
//! A field the scaffold cannot render as a text box (a boolean, an enum, a list, an object) stops the run with its
//! name, so the screen is never generated with a silently missing input.

use anyhow::{Result, bail};
use serde::Serialize;

use super::openapi::{self, Document, Node, Operation};

/// How a field is typed on the wire, which decides its validation and its conversion at submit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Uuid,
    Number,
    Integer,
}

/// One input of the command, as the templates render it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    /// The wire name, which is also the form field and the orval variable (`unitPrice`).
    pub name: String,
    /// The default English label (`Unit price`).
    pub label: String,
    /// `text` or `number`: the `<Input kind>` the View renders.
    pub input: &'static str,
    /// The zod schema for the string the input holds; `{msg}` is the localized error expression.
    pub rule: String,
    /// The expression that turns `values.<name>` into the wire value.
    pub value: String,
    /// A path parameter travels beside `data`, not inside it.
    pub in_path: bool,
    /// The default English error copy.
    pub error: String,
}

impl Field {
    fn new(name: &str, kind: Kind, required: bool, nullable: bool, in_path: bool) -> Field {
        let empty = if nullable { "null" } else { "undefined" };
        let v = format!("values.{name}");
        let (rule, value, error) = match (kind, required) {
            (Kind::Text, true) => (
                "z.string().trim().min(1, {msg})".to_string(),
                v.clone(),
                "This field is required.",
            ),
            (Kind::Text, false) => ("z.string()".to_string(), v.clone(), "Check this field."),
            (Kind::Uuid, true) => ("z.uuid({msg})".to_string(), v.clone(), "Enter a valid id."),
            (Kind::Uuid, false) => (
                "z.union([z.literal(\"\"), z.uuid({msg})])".to_string(),
                format!("{v} === \"\" ? {empty} : {v}"),
                "Enter a valid id.",
            ),
            (Kind::Number | Kind::Integer, required) => {
                let check = if kind == Kind::Integer {
                    "Number.isInteger"
                } else {
                    "Number.isFinite"
                };
                let accepts = if required {
                    format!("v.trim() !== \"\" && {check}(Number(v))")
                } else {
                    format!("v.trim() === \"\" || {check}(Number(v))")
                };
                (
                    format!("z.string().refine((v) => {accepts}, {{msg}})"),
                    if required {
                        format!("Number({v})")
                    } else {
                        format!("{v}.trim() === \"\" ? {empty} : Number({v})")
                    },
                    if kind == Kind::Integer {
                        "Enter a whole number."
                    } else {
                        "Enter a number."
                    },
                )
            }
        };
        Field {
            name: name.to_string(),
            label: label(name),
            input: if matches!(kind, Kind::Number | Kind::Integer) {
                "number"
            } else {
                "text"
            },
            rule,
            value,
            in_path,
            error: error.to_string(),
        }
    }
}

/// `unitPrice` → `Unit price`.
fn label(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if index == 0 {
            out.push(c.to_ascii_uppercase());
        } else if c.is_ascii_uppercase() {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else if c == '_' || c == '-' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// `title:string,price:number,ownerId:uuid` → body fields, every one required.
pub fn parse(spec: &str) -> Result<Vec<Field>> {
    let mut fields = Vec::new();
    for pair in spec.split(',').map(str::trim).filter(|pair| !pair.is_empty()) {
        let Some((name, kind)) = pair.split_once(':') else {
            bail!("--fields: `{pair}` is not `name:type` (types: string, number, integer, uuid)");
        };
        let name = name.trim();
        if !name.starts_with(|c: char| c.is_ascii_alphabetic()) || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("--fields: `{name}` is not a field name; use the wire name (camelCase letters and digits)");
        }
        let kind = match kind.trim() {
            "string" | "text" => Kind::Text,
            "number" => Kind::Number,
            "integer" | "int" => Kind::Integer,
            "uuid" | "guid" => Kind::Uuid,
            other => bail!("--fields: `{other}` is not a field type (string, number, integer, uuid)"),
        };
        fields.push(Field::new(name, kind, true, false, false));
    }
    if fields.is_empty() {
        bail!("--fields names no field; pass `name:type` pairs, e.g. --fields title:string,price:number");
    }
    Ok(fields)
}

/// The command's inputs from its operation: path parameters first, then the JSON body's properties.
pub fn from_operation(doc: &Document, operation: &Operation, slice: &str) -> Result<Vec<Field>> {
    if let [(name, _), ..] = operation.parameters("query").as_slice() {
        bail!(
            "{slice} takes query parameters ({name}, ...), which the form scaffold does not bind; pass --fields for \
             the body and wire the parameters by hand"
        );
    }
    let mut fields = Vec::new();
    let mut unsupported = Vec::new();
    for (name, schema) in operation.parameters("path") {
        match kind(doc.resolve(schema)) {
            Some(kind) => fields.push(Field::new(&name, kind, true, false, true)),
            None => unsupported.push(name),
        }
    }
    if let Some(body) = operation.body() {
        for (name, schema, required) in openapi::properties(body) {
            if fields.iter().any(|f| f.name == name) {
                continue;
            }
            let nullable = openapi::is_nullable(schema);
            match kind(doc.resolve(schema)) {
                Some(kind) => fields.push(Field::new(&name, kind, required && !nullable, nullable, false)),
                None => unsupported.push(name),
            }
        }
    }
    if !unsupported.is_empty() {
        bail!(
            "{slice}'s input has fields the form scaffold renders no text box for: {}. Scaffold the others with \
             --fields and add those controls by hand",
            unsupported.join(", ")
        );
    }
    Ok(fields)
}

fn kind(schema: &Node) -> Option<Kind> {
    if schema.get("enum").is_some() {
        return None;
    }
    match openapi::type_of(schema)? {
        "string" if schema.get("format").and_then(Node::as_str) == Some("uuid") => Some(Kind::Uuid),
        "string" => Some(Kind::Text),
        "integer" => Some(Kind::Integer),
        "number" => Some(Kind::Number),
        _ => None,
    }
}

/// What a list screen reads from the page its slice returns.
#[derive(Debug, PartialEq, Serialize)]
pub struct ListShape {
    /// The Output property holding the page (`products`).
    pub collection: String,
    /// The generated row type (`ProductView`).
    pub row: String,
    /// The row property a line shows: its first text property other than `id`, else `id`.
    pub display: String,
    /// The row's unique key property, when it has one.
    pub key: Option<String>,
}

/// The page shape of a list operation's response: `{ <collection>: { items: <Row>[] } }`.
pub fn list_shape(doc: &Document, operation: &Operation) -> Option<ListShape> {
    let output = operation.response()?;
    openapi::properties(output)
        .into_iter()
        .find_map(|(collection, page, _)| {
            let page = doc.resolve(page);
            let items = page.get("properties")?.get("items")?;
            let item = items.get("items")?;
            let row = openapi::schema_name(item)?;
            let props = openapi::properties(doc.resolve(item));
            let key = props
                .iter()
                .find(|(name, _, _)| name == "id")
                .map(|(name, _, _)| name.clone());
            let display = props
                .iter()
                .find(|(name, schema, _)| name != "id" && openapi::type_of(doc.resolve(schema)) == Some("string"))
                .or_else(|| props.first())
                .map(|(name, _, _)| name.clone())?;
            Some(ListShape {
                collection,
                row,
                display,
                key,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_fields_flag() {
        let fields = parse("title:string, price:number,ownerId:uuid").unwrap();
        assert_eq!(
            fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            ["title", "price", "ownerId"]
        );
        assert_eq!(fields[1].value, "Number(values.price)");
        assert_eq!(fields[1].input, "number");
        assert_eq!(fields[2].label, "Owner id");
        assert!(parse("title").is_err() && parse("x:bool").is_err() && parse("").is_err());
    }

    #[test]
    fn reads_path_parameters_and_the_body_and_refuses_what_it_cannot_render() {
        let doc = Document::parse(
            r##"{
            "paths": { "/p/{id}": { "put": {
                "operationId": "UpdateProduct",
                "parameters": [{ "name": "id", "in": "path", "schema": { "type": "string", "format": "uuid" } }],
                "requestBody": { "content": { "application/json": { "schema": { "type": "object",
                    "required": ["name", "price"],
                    "properties": { "name": { "type": "string" }, "price": { "type": "number" },
                        "note": { "type": ["string", "null"] }, "stock": { "type": "integer" } } } } } }
            } },
            "/q": { "post": { "operationId": "Toggle", "requestBody": { "content": { "application/json": { "schema": {
                "type": "object", "properties": { "on": { "type": "boolean" } } } } } } } } }
        }"##,
        )
        .unwrap();
        let op = doc.operation("UpdateProduct").unwrap();
        let fields = from_operation(&doc, &op, "UpdateProduct").unwrap();
        let names: Vec<(&str, bool)> = fields.iter().map(|f| (f.name.as_str(), f.in_path)).collect();
        assert_eq!(
            names,
            [
                ("id", true),
                ("name", false),
                ("price", false),
                ("note", false),
                ("stock", false)
            ]
        );
        assert_eq!(fields[3].rule, "z.string()");
        assert_eq!(
            fields[4].value,
            "values.stock.trim() === \"\" ? undefined : Number(values.stock)"
        );
        assert!(fields[4].rule.contains("Number.isInteger"));

        let toggle = doc.operation("Toggle").unwrap();
        let error = from_operation(&doc, &toggle, "Toggle").unwrap_err().to_string();
        assert!(error.contains("on") && error.contains("--fields"), "{error}");
    }
}
