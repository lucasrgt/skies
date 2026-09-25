//! Rendering the crud templates: the tokens each slice and the entity's view record are filled with.

use super::super::text;
use super::Crud;

/// A scalar field on the wire: `Guid Id` in the view record, `e.Id` in its projection.
pub(super) struct ViewField {
    pub name: String,
    pub ty: String,
}

/// One generated slice: its class name, the template it renders from, and whether it writes.
pub(super) struct Slice {
    pub name: String,
    pub template: &'static str,
    pub writes: bool,
}

impl Crud {
    /// The slices in mapping order: reads first (`ListProducts`, `LookupProduct`, `LookupMyProduct`), then writes.
    /// The list is named for the collection, the rest for the one row they act on.
    pub(super) fn slices(&self) -> Vec<Slice> {
        let e = &self.entity;
        let slice = |name: String, template: &'static str, writes: bool| Slice { name, template, writes };
        let mut slices = vec![
            slice(format!("List{}", self.plural), "crud/List__PLURAL__.cs.cstmpl", false),
            slice(format!("Lookup{e}"), "crud/Lookup__ENTITY__.cs.cstmpl", false),
        ];
        if self.has_user_id {
            slices.push(slice(
                format!("LookupMy{e}"),
                "crud/LookupMy__ENTITY__.cs.cstmpl",
                false,
            ));
        }
        slices.extend([
            slice(format!("Create{e}"), "crud/Create__ENTITY__.cs.cstmpl", true),
            slice(format!("Update{e}"), "crud/Update__ENTITY__.cs.cstmpl", true),
            slice(format!("Delete{e}"), "crud/Delete__ENTITY__.cs.cstmpl", true),
        ]);
        slices
    }

    fn stamps(&self) -> bool {
        self.has_created_at || self.has_updated_at
    }

    /// The structural tokens first (the longer `__ENTITY_LOWER__` before `__ENTITY__`), then the spliced field
    /// fragments, then the app tokens, exactly as the auth blueprint's token pass.
    pub(super) fn render(&self, template: &str, writes: bool) -> String {
        let pick = |on: bool, value: &'static str| if on { value } else { "" };
        let create_params = format!(
            "{}{}",
            pick(self.has_user_id, "ICurrentUser current, "),
            pick(self.stamps(), "TimeProvider clock, ")
        );
        let create_args = format!(
            "{}{}",
            pick(self.has_user_id, "current, "),
            pick(self.stamps(), "clock, ")
        );
        let order = if self.has_created_at {
            "OrderByDescending(e => e.CreatedAt).ThenBy(e => e.Id)"
        } else {
            "OrderBy(e => e.Id)"
        };
        let article = text::with_article(&self.entity);
        let article_cap = capitalize(&article);
        let body = text::fill(
            &text::normalize_newlines(template),
            &[
                ("__MODULE__", &self.module),
                ("__ENTITY_LOWER__", &text::hyphenate(&self.entity)),
                ("__ROUTE__", &text::hyphenate(&self.plural)),
                ("__A_ENTITY_CAP__", &article_cap),
                ("__A_ENTITY__", &article),
                ("__ENTITY__", &self.entity),
                ("__PLURAL__", &self.plural),
                ("__INPUT_FIELDS__", &self.input_fields()),
                ("__CHANGES_ARGS__", &self.changes_args()),
                ("__OPEN_ARGS__", &self.open_args()),
                ("__REQUIRED_CHECKS__", &self.required_checks()),
                ("__UPDATE_ARGS__", &self.update_args()),
                ("__VIEW_FIELDS__", &self.view_fields()),
                ("__VIEW_ARGS__", &self.view_args()),
                (
                    "__HIDDEN__",
                    pick(self.tenant_scoped, "\n/// The owning org stays off the wire."),
                ),
                ("__WITHIN__", pick(self.tenant_scoped, " within the caller's org")),
                (
                    "__FOREIGN__",
                    pick(
                        self.tenant_scoped,
                        "\n/// An id from another org is a not-found, never a hint that the row exists.",
                    ),
                ),
                (
                    "__STAMPED__",
                    pick(
                        self.tenant_scoped,
                        "\n/// The org is stamped by the DbContext, never taken from the request.",
                    ),
                ),
                ("__POSTURE__", self.posture(writes)),
                (
                    "__AUTH_USING__",
                    pick(self.has_user_id, "using Skies.Framework.Auth;\n\n"),
                ),
                ("__CREATE_PARAMS__", &create_params),
                ("__CREATE_ARGS__", &create_args),
                ("__CLOCK_PARAM__", pick(self.has_updated_at, "TimeProvider clock, ")),
                ("__CLOCK_ARG__", pick(self.has_updated_at, "clock, ")),
                ("__ORDER__", order),
            ],
        );
        text::replace_app_tokens(&body, &self.app_name, &self.app_lower)
    }

    /// A slice's own authorization posture: none when a group above it decides. Writes to an app-wide entity always
    /// map under the module's admin group, so only the reads (and a tenant-scoped entity's writes) can fall back to
    /// their own fail-closed posture.
    fn posture(&self, writes: bool) -> &'static str {
        if self.group_decides || (writes && !self.tenant_scoped) {
            super::super::scaffold::INHERITED_POSTURE
        } else {
            super::super::scaffold::OWN_POSTURE
        }
    }

    /// `string Name, decimal Price`: the request carries exactly the fields the entity's factory takes.
    fn input_fields(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("{} {}", f.ty, f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `changes.Name, changes.Price, changes.Version`: the body's fields, after the route's id.
    fn changes_args(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("changes.{}", f.name))
            .chain(std::iter::once("changes.Version".to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A body that leaves a field out binds it as null whatever the record declares, so every field that cannot
    /// hold null is checked before the entity sees it: a missing field is the caller's `400`, never a `500` from the
    /// save. Value types bind their default instead, which the entity's own invariants judge.
    fn required_checks(&self) -> String {
        let checks: Vec<String> = self
            .scalars
            .iter()
            .filter(|f| !f.ty.ends_with('?') && !VALUE_TYPES.contains(&f.ty.as_str()))
            .map(|f| {
                let value = if f.ty == "string" {
                    format!("input.{}", f.name)
                } else {
                    format!("(object?)input.{}", f.name)
                };
                format!(
                    "\n            .Check({value} is not null, \"{}\", {}ErrorCodes.{}FieldRequired, \"is required\")",
                    camel(&f.name),
                    self.module,
                    self.entity
                )
            })
            .collect();
        if checks.is_empty() {
            return String::new();
        }
        format!(
            "        var missing = new Validation(){};\n        if (missing.Failed)\n            return missing.ToError();\n\n",
            checks.concat()
        )
    }

    fn field_args(&self) -> Vec<String> {
        self.scalars.iter().map(|f| format!("input.{}", f.name)).collect()
    }

    /// The positional convention `Open` follows: id, the fields in declaration order, the owner, the clock.
    pub(super) fn open_args(&self) -> String {
        let mut args = vec!["Guid.NewGuid()".to_string()];
        args.extend(self.field_args());
        if self.has_user_id {
            args.push("current.UserId".to_string());
        }
        if self.stamps() {
            args.push("clock.GetUtcNow().UtcDateTime".to_string());
        }
        args.join(", ")
    }

    fn update_args(&self) -> String {
        let mut args = self.field_args();
        if self.has_updated_at {
            args.push("clock.GetUtcNow().UtcDateTime".to_string());
        }
        args.join(", ")
    }

    fn view_fields(&self) -> String {
        self.view
            .iter()
            .map(|f| format!("{} {}", f.ty, f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn view_args(&self) -> String {
        self.view
            .iter()
            .map(|f| format!("e.{}", f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The C# value types a crud field is commonly declared as: they never bind as null.
const VALUE_TYPES: &[&str] = &[
    "bool", "byte", "sbyte", "short", "ushort", "int", "uint", "long", "ulong", "float", "double", "decimal", "char",
    "Guid", "DateTime", "DateTimeOffset", "DateOnly", "TimeOnly", "TimeSpan",
];

/// The JSON name ASP.NET Core binds a property from: `Name` → `name`.
fn camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
