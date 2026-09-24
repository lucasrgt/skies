//! Rendering the crud templates: the tokens each slice and the entity's view record are filled with.

use super::super::text;
use super::Crud;

/// A scalar field on the wire: `Guid Id` in the view record, `e.Id` in its projection.
pub(super) struct ViewField {
    pub name: String,
    pub ty: String,
}

impl Crud {
    pub(super) fn slices(&self) -> Vec<String> {
        let e = &self.entity;
        let mut slices = vec![format!("List{e}"), format!("Lookup{e}")];
        if self.has_user_id {
            slices.push(format!("LookupMy{e}"));
        }
        slices.extend([format!("Create{e}"), format!("Update{e}"), format!("Delete{e}")]);
        slices
    }

    fn stamps(&self) -> bool {
        self.has_created_at || self.has_updated_at
    }

    /// The structural tokens first (the longer `__ENTITY_LOWER__` before `__ENTITY__`), then the spliced field
    /// fragments, then the app tokens, exactly as the auth blueprint's token pass.
    pub(super) fn render(&self, template: &str) -> String {
        let hyphen = text::hyphenate(&self.entity);
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
        let posture = if self.group_decides {
            super::super::scaffold::INHERITED_POSTURE
        } else {
            super::super::scaffold::OWN_POSTURE
        };
        let hidden = if self.tenant_scoped {
            "the owning org and the concurrency token"
        } else {
            "persistence details such as the concurrency token"
        };
        let body = text::fill(
            &text::normalize_newlines(template),
            &[
                ("__MODULE__", &self.module),
                ("__ENTITY_LOWER__", &hyphen),
                ("__A_ENTITY_CAP__", &article_cap),
                ("__A_ENTITY__", &article),
                ("__ENTITY__", &self.entity),
                ("__PLURAL__", &self.plural),
                ("__INPUT_FIELDS__", &self.input_fields()),
                ("__CHANGES_ARGS__", &self.changes_args()),
                ("__OPEN_ARGS__", &self.open_args()),
                ("__UPDATE_ARGS__", &self.update_args()),
                ("__VIEW_FIELDS__", &self.view_fields()),
                ("__VIEW_ARGS__", &self.view_args()),
                ("__HIDDEN__", hidden),
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
                ("__POSTURE__", posture),
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

    /// `string Name, decimal Price`: the request carries exactly the fields the entity's factory takes.
    fn input_fields(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("{} {}", f.ty, f.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `changes.Name, changes.Price`: the body's fields, after the route's id.
    fn changes_args(&self) -> String {
        self.scalars
            .iter()
            .map(|f| format!("changes.{}", f.name))
            .collect::<Vec<_>>()
            .join(", ")
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

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
