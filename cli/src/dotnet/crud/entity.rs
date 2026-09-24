//! The domain half of `g crud`: the entity members the Create and Update slices call.
//!
//! An `[Entity]` has no public setter and no public constructor (SKY0014), so a slice cannot write columns. The
//! generator therefore writes the two intention-revealing members the slices need into the entity itself, from
//! the fields it reads off the entity's properties, and the slices only call them:
//!
//! - `Open(Guid id, <fields>[, Guid userId][, DateTime now])` — replaces the bare `Open(Guid id)` that
//!   `g entity` scaffolds (recognized by its exact text). An `Open` the author already wrote is kept as-is and
//!   the Create slice calls it with that same positional convention; a mismatch is a compile error on one line.
//! - `Update(<fields>[, DateTime now])` — added when the entity has no `Update`; kept when it has one.
//! - `RowVersion` — the concurrency token (SKY0026) for the tracked update and delete, added when the entity
//!   declares none.
//!
//! Every generated member returns through the entity's private `EnsureValid`, so the invariants the author adds
//! there hold for CRUD writes too. The edits are anchored on the private constructor and the invariant funnel,
//! which the entity scaffold supplies; the file's newline style is preserved.

use std::sync::LazyLock;

use regex::Regex;

use super::Field;

/// What the entity's shape says the generated members must carry besides the domain fields.
pub(super) struct Shape<'a> {
    pub entity: &'a str,
    pub fields: &'a [Field],
    pub has_user_id: bool,
    pub has_created_at: bool,
    pub has_updated_at: bool,
}

/// The members `complete` added, for the generator's summary.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Added {
    pub open: bool,
    pub update: bool,
    pub row_version: bool,
    /// The author's own `Open` was kept; the Create slice calls it positionally.
    pub kept_open: bool,
}

/// The C# keywords a field name can collide with once camel-cased (`Event` → `event`).
const KEYWORDS: &str = "\
     abstract base bool case catch char checked class const continue decimal default delegate do double else enum \
     event explicit extern false finally fixed float for foreach goto if implicit in int interface internal is \
     lock long namespace new null object operator out override params private protected public readonly ref \
     return sbyte sealed short sizeof stackalloc static string struct switch this throw true try typeof uint \
     ulong unchecked unsafe ushort using virtual void volatile while";

static CONCURRENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bRowVersion\b|\[(?:[A-Za-z.]*\.)?(?:Timestamp|ConcurrencyCheck)\b").unwrap());

/// `Name` becomes the parameter `name`; a C# keyword (`Event` → `event`) is escaped as `@event`.
pub(super) fn parameter(name: &str) -> String {
    let mut chars = name.chars();
    let lowered: String = match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    };
    if KEYWORDS.split_whitespace().any(|k| k == lowered) {
        format!("@{lowered}")
    } else {
        lowered
    }
}

impl Shape<'_> {
    fn stamps(&self) -> bool {
        self.has_created_at || self.has_updated_at
    }

    fn field_params(&self) -> Vec<String> {
        self.fields
            .iter()
            .map(|f| format!("{} {}", f.ty, parameter(&f.name)))
            .collect()
    }

    fn open_signature(&self) -> String {
        let mut params = vec!["Guid id".to_string()];
        params.extend(self.field_params());
        if self.has_user_id {
            params.push("Guid userId".to_string());
        }
        if self.stamps() {
            params.push("DateTime now".to_string());
        }
        params.join(", ")
    }

    fn open_member(&self) -> String {
        let e = self.entity;
        let mut inits = vec!["Id = id".to_string()];
        inits.extend(
            self.fields
                .iter()
                .map(|f| format!("{} = {}", f.name, parameter(&f.name))),
        );
        if self.has_user_id {
            inits.push("UserId = userId".to_string());
        }
        if self.has_created_at {
            inits.push("CreatedAt = now".to_string());
        }
        if self.has_updated_at {
            inits.push("UpdatedAt = now".to_string());
        }
        let doc = format!(
            "    /// <summary>Open a new {e} with its identity and fields. Creation returns through\n    \
             /// <see cref=\"EnsureValid\"/>, so a {e} that breaks an invariant is refused before it\n    \
             /// exists.</summary>\n"
        );
        let signature = format!("    public static Result<{e}> Open({}) =>\n", self.open_signature());
        let body = format!("        new {e} {{ {} }}.EnsureValid();\n", inits.join(", "));
        [doc, signature, body].concat()
    }

    fn update_member(&self) -> String {
        let e = self.entity;
        let mut params = self.field_params();
        let mut changes: Vec<(String, String)> = self
            .fields
            .iter()
            .map(|f| (f.name.clone(), parameter(&f.name)))
            .collect();
        if self.has_updated_at {
            params.push("DateTime now".to_string());
            changes.push(("UpdatedAt".to_string(), "now".to_string()));
        }
        let proposed = changes
            .iter()
            .map(|(name, value)| format!("        proposed.{name} = {value};\n"))
            .collect::<String>();
        let applied = changes
            .iter()
            .map(|(name, _)| format!("        {name} = proposed.{name};\n"))
            .collect::<String>();
        let signature = format!("    public Result<{e}> Update({})\n    {{\n", params.join(", "));
        [
            format!("    /// <summary>Validate proposed values before changing this {e}.</summary>\n"),
            signature,
            format!("        var proposed = ({e})MemberwiseClone();\n"),
            proposed,
            "        var validation = proposed.EnsureValid();\n".to_string(),
            "        if (validation.IsFailure) return validation.Error;\n".to_string(),
            applied,
            "        return this;\n    }\n".to_string(),
        ]
        .concat()
    }

    /// The exact `Open` that `g entity` scaffolds: the generator's own output, safe to replace.
    fn skeleton_open(&self) -> String {
        let e = self.entity;
        format!(
            "    /// <summary>Open a new {e} with the given identity.</summary>\n    \
             public static Result<{e}> Open(Guid id) =>\n        new {e} {{ Id = id }}.EnsureValid();\n"
        )
    }
}

const ROW_VERSION: &str = concat!(
    "    /// <summary>The optimistic-concurrency token: a concurrent update or delete of the same\n",
    "    /// row fails loudly with DbUpdateConcurrencyException instead of silently erasing the other\n",
    "    /// write.</summary>\n",
    "    [System.ComponentModel.DataAnnotations.Timestamp]\n",
    "    public byte[]? RowVersion { get; private set; }\n",
);

/// Adds the members the CRUD slices call. Fails (with the reason) when the entity lacks the scaffold anchors: the private parameterless constructor and the private `EnsureValid` funnel.
pub(super) fn complete(source: &str, shape: &Shape) -> Result<(String, Added), String> {
    let e = regex::escape(shape.entity);
    let nl = super::text::newline_of(source);
    let mut text = super::text::normalize_newlines(source);
    let funnel = Regex::new(&format!(r"(?m)^[ \t]*private\s+Result<{e}>\s+EnsureValid\s*\(")).unwrap();
    let ctor = Regex::new(&format!(r"(?m)^[ \t]*private\s+{e}\s*\(\s*\)")).unwrap();
    if !funnel.is_match(&text) || !ctor.is_match(&text) {
        return Err(format!(
            "{} has no private `{0}()` constructor or private `Result<{0}> EnsureValid()` funnel",
            shape.entity
        ));
    }

    let mut added = Added::default();
    if !CONCURRENCY.is_match(&text) {
        text = insert_above(&text, &ctor, ROW_VERSION);
        added.row_version = true;
    }

    let skeleton = shape.skeleton_open();
    let any_open = Regex::new(&format!(r"static\s+Result<{e}>\s+Open\s*\(")).unwrap();
    if text.contains(&skeleton) {
        text = text.replacen(&skeleton, &shape.open_member(), 1);
        added.open = true;
    } else if any_open.is_match(&text) {
        added.kept_open = true;
    } else {
        text = insert_above(&text, &funnel, &shape.open_member());
        added.open = true;
    }

    let any_update = Regex::new(r"(?:Result<[A-Za-z0-9_]+>|void)\s+Update\s*\(").unwrap();
    if !any_update.is_match(&text) {
        text = insert_above(&text, &funnel, &shape.update_member());
        added.update = true;
    }

    Ok((text.replace('\n', nl), added))
}

/// Inserts `member` and a blank line above the first line `anchor` matches, and above the comments and
/// attributes that belong to that line, so a generated member never splits a declaration from its doc.
fn insert_above(text: &str, anchor: &Regex, member: &str) -> String {
    let found = anchor.find(text).expect("anchor checked before inserting");
    let mut at = text[..found.start()].rfind('\n').map_or(0, |i| i + 1);
    while at > 0 {
        let previous = text[..at - 1].rfind('\n').map_or(0, |i| i + 1);
        let line = text[previous..at - 1].trim_start();
        if line.starts_with("//") || line.starts_with('[') {
            at = previous;
        } else {
            break;
        }
    }
    format!("{}{member}\n{}", &text[..at], &text[at..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKELETON: &str = concat!(
        "[Entity]\npublic class Product : ITenantScoped\n{\n",
        "    public Guid Id { get; private set; }\n\n",
        "    public string Name { get; private set; } = \"\";\n\n",
        "    // Parameterless and private: EF.\n    private Product() { }\n\n",
        "    /// <summary>Open a new Product with the given identity.</summary>\n",
        "    public static Result<Product> Open(Guid id) =>\n        new Product { Id = id }.EnsureValid();\n\n",
        "    // Add intention-revealing methods here.\n\n",
        "    // The single invariant funnel.\n    private Result<Product> EnsureValid()\n",
        "    {\n        return this;\n    }\n}\n",
    );

    fn shape(fields: &[Field]) -> Shape<'_> {
        Shape {
            entity: "Product",
            fields,
            has_user_id: false,
            has_created_at: false,
            has_updated_at: true,
        }
    }

    fn name() -> Vec<Field> {
        vec![Field {
            name: "Name".into(),
            ty: "string".into(),
        }]
    }

    #[test]
    fn the_skeleton_gains_a_field_factory_an_update_and_a_concurrency_token() {
        let fields = name();
        let (text, added) = complete(SKELETON, &shape(&fields)).unwrap();

        assert_eq!(
            added,
            Added {
                open: true,
                update: true,
                row_version: true,
                kept_open: false
            }
        );
        assert!(text.contains(
            "public static Result<Product> Open(Guid id, string name, DateTime now) =>\n        \
             new Product { Id = id, Name = name, UpdatedAt = now }.EnsureValid();"
        ));
        assert!(!text.contains("Open(Guid id) =>"));
        assert!(text.contains("var proposed = (Product)MemberwiseClone();"));
        assert!(text.contains("proposed.Name = name;"));
        assert!(text.contains("var validation = proposed.EnsureValid();"));
        assert!(text.contains("if (validation.IsFailure) return validation.Error;\n        Name = proposed.Name;"));
        assert!(text.contains("public byte[]? RowVersion { get; private set; }\n\n    // Parameterless and private"));
        assert!(!text.contains("{ get; set; }"));
    }

    #[test]
    fn the_authors_own_members_are_kept_and_a_second_run_changes_nothing() {
        let fields = name();
        let (once, _) = complete(SKELETON, &shape(&fields)).unwrap();
        let (twice, added) = complete(&once, &shape(&fields)).unwrap();

        assert_eq!(once, twice);
        assert_eq!(
            added,
            Added {
                kept_open: true,
                ..Added::default()
            }
        );
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let fields = name();
        let (text, _) = complete(&SKELETON.replace('\n', "\r\n"), &shape(&fields)).unwrap();
        assert!(!text.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn an_entity_without_the_funnel_is_refused() {
        let fields = name();
        let data_bag = "public class Product { public string Name { get; set; } }";
        assert!(complete(data_bag, &shape(&fields)).is_err());
    }

    #[test]
    fn keyword_fields_become_escaped_parameters() {
        assert_eq!(parameter("Event"), "@event");
        assert_eq!(parameter("DueAt"), "dueAt");
    }
}
