//! Names a generator turns into C#: the app's namespace, a module, a slice, an entity, a value object, a hub.
//!
//! Each becomes an identifier in generated code (a namespace segment, a class, a file), so a name C# cannot spell as
//! a type (`class`, `2fa`, `my-app`) is refused up front with the rule it broke, instead of rendering an app that
//! fails to compile with a CS1001 far from the command that caused it.

/// C#'s reserved keywords: never an identifier without an `@`, which generated code does not write.
const RESERVED: &[&str] = &[
    "abstract",
    "as",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "null",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

/// Contextual keywords that C# refuses, or reads as something else, where a type name goes (`record Foo`, `var x`).
const NOT_A_TYPE_NAME: &[&str] = &[
    "dynamic",
    "file",
    "managed",
    "nint",
    "notnull",
    "nuint",
    "record",
    "required",
    "scoped",
    "unmanaged",
    "var",
];

/// Names the generated code writes unqualified (`Result<T>`, `Validation`, `Task`, `Guid`, `AppDb`, …) or that
/// open a namespace every file resolves through (`System`, `Microsoft`, `Skies`). A module, entity, slice, value
/// object, or hub with one of these names shadows it, and the module stops compiling far from the command.
const TAKEN_BY_THE_FRAMEWORK: &[&str] = &[
    "AppDb",
    "AppJson",
    "CancellationToken",
    "DateTime",
    "DateTimeOffset",
    "Error",
    "Exception",
    "Guid",
    "HttpContext",
    "IResult",
    "Microsoft",
    "Module",
    "Page",
    "Platform",
    "Program",
    "Result",
    "Results",
    "Skies",
    "Slice",
    "System",
    "Task",
    "TimeProvider",
    "Validation",
];

/// Why `name` cannot be a C# identifier for a `kind` ("module", "slice", ...), or `None` when it can.
pub fn identifier_problem(kind: &str, name: &str) -> Option<String> {
    let mut chars = name.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    if !starts_well || !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return Some(format!(
            "'{name}' is not a valid {kind} name: it becomes a C# identifier, so use letters, digits, and \
             underscores, starting with a letter (e.g. Billing, CreateInvoice)."
        ));
    }
    if is_keyword(name) {
        return Some(format!(
            "'{name}' is not a valid {kind} name: it is a C# keyword, so the generated code would not compile. \
             Pick another name, such as {}.",
            suggestion(name)
        ));
    }
    if kind != "application" && TAKEN_BY_THE_FRAMEWORK.contains(&name) {
        return Some(format!(
            "'{name}' is not a valid {kind} name: generated code already uses `{name}` (a framework or runtime type              it names unqualified), so the module would not compile. Pick a domain name, such as {name}Record."
        ));
    }
    None
}

/// Why a dotted application name (`Acme`, `Acme.Billing`) cannot be the app's root namespace, or `None`.
pub fn namespace_problem(name: &str) -> Option<String> {
    if name.is_empty()
        || name
            .split('.')
            .any(|part| identifier_problem("application", part).is_some())
    {
        let keyword = name.split('.').find(|part| is_keyword(part));
        return Some(match keyword {
            Some(part) => format!(
                "'{name}' is not a valid application name: '{part}' is a C# keyword, so the generated solution would \
                 not compile. Pick another name, such as {}.",
                suggestion(part)
            ),
            None => format!(
                "'{name}' is not a valid application name — use a C# namespace such as Acme or Acme.Billing \
                 (letters, digits, and underscores; dots between parts)."
            ),
        });
    }
    None
}

/// Keywords are case-sensitive in C#: `class` is one, `Class` is an ordinary name.
fn is_keyword(name: &str) -> bool {
    RESERVED.contains(&name) || NOT_A_TYPE_NAME.contains(&name)
}

/// A nearby legal name: the keyword capitalized and qualified, so `class` suggests `ClassApp`.
fn suggestion(keyword: &str) -> String {
    let mut chars = keyword.chars();
    let capitalized: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    format!("{capitalized}App")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_pascal_names_are_fine() {
        for name in ["Billing", "CreateInvoice", "Order2", "_Legacy", "Class", "Record"] {
            assert_eq!(identifier_problem("module", name), None, "{name}");
        }
        assert_eq!(namespace_problem("Acme.Billing"), None);
    }

    #[test]
    fn keywords_are_refused_with_the_reason() {
        for name in ["class", "namespace", "int", "record", "var"] {
            let problem = identifier_problem("slice", name).expect(name);
            assert!(problem.contains("C# keyword"), "{problem}");
        }
        let app = namespace_problem("class").expect("class");
        assert!(
            app.contains("'class' is a C# keyword") && app.contains("ClassApp"),
            "{app}"
        );
        assert!(namespace_problem("Acme.event").expect("Acme.event").contains("'event'"));
    }

    #[test]
    fn framework_type_names_are_refused_for_generated_types() {
        for name in ["Validation", "Result", "Error", "Task", "Guid", "System", "AppDb"] {
            let problem = identifier_problem("entity", name).expect(name);
            assert!(problem.contains("generated code already uses"), "{problem}");
        }
        assert_eq!(identifier_problem("entity", "Validations"), None);
        assert_eq!(namespace_problem("Result"), None, "an app may be called Result");
    }

    #[test]
    fn invalid_identifiers_are_refused() {
        for name in ["", "2fa", "my-app", "has space", "Café!"] {
            assert!(identifier_problem("entity", name).is_some(), "{name:?}");
        }
        for name in ["", "Acme.", ".Acme", "my-app", "Acme..Billing"] {
            assert!(namespace_problem(name).is_some(), "{name:?}");
        }
    }
}
