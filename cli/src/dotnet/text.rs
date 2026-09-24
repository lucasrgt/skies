//! Text edits shared by the generators.
//!
//! The generators edit files a person owns (`Program.cs`, a module, a csproj), so every edit is anchored on a
//! literal line the scaffold is known to contain and is otherwise a no-op. These helpers keep the anchor
//! semantics identical to the 4.x CLI: first occurrence only, ordinal comparison, and the target file's own
//! newline style preserved.

use std::path::Path;

use anyhow::{Context, Result};

/// The newline a file already uses, so an inserted line never mixes CRLF into an LF file or the reverse.
pub fn newline_of(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Templates may be checked out with CRLF on Windows; rendering always starts from LF.
pub fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Replaces the first occurrence only. Anchors appear once in a scaffolded file, and a second match is more
/// likely a user's copy of the line than a second anchor.
pub fn replace_first(text: &str, find: &str, replacement: &str) -> String {
    match text.find(find) {
        Some(at) => format!("{}{}{}", &text[..at], replacement, &text[at + find.len()..]),
        None => text.to_string(),
    }
}

/// Inserts `lines` on their own line just above the first `</ItemGroup>`, where the scaffold keeps its
/// package references, so a new reference sits beside the existing ones with the same indentation.
pub fn insert_before_closing_item_group(text: &str, lines: &str, nl: &str) -> String {
    let Some(at) = text.find("</ItemGroup>") else {
        return text.to_string();
    };
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    format!("{}{}{}{}", &text[..line_start], lines, nl, &text[line_start..])
}

/// Inserts a block just before the final `</Project>`, for item groups the scaffold does not have yet.
pub fn insert_before_project_end(text: &str, block: &str) -> String {
    match text.rfind("</Project>") {
        Some(at) => format!("{}{}{}", &text[..at], block, &text[at..]),
        None => text.to_string(),
    }
}

/// `OrderLine` becomes `order-line`: the URL segment and error-code prefix for an entity.
pub fn hyphenate(pascal: &str) -> String {
    let mut out = String::with_capacity(pascal.len() + 4);
    for (i, c) in pascal.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// `Invoice` becomes `an Invoice`, `Product` becomes `a Product`: the article generated prose puts before an
/// entity name. Judged by sound for the common English cases (`a User`, `an Hour` is not attempted).
pub fn with_article(noun: &str) -> String {
    let lower = noun.to_lowercase();
    let vowel = lower.starts_with(['a', 'e', 'i', 'o', 'u']);
    let sounds_like_you = ["us", "uni", "uti", "ure", "eu", "one"]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let article = if vowel && !sounds_like_you { "an" } else { "a" };
    format!("{article} {noun}")
}

/// `Invoice` becomes `Invoices`, `Category` `Categories`, `Address` `Addresses`: the DbSet name for an entity.
pub fn plural(noun: &str) -> String {
    let lower = noun.to_lowercase();
    let consonant_y = lower.ends_with('y') && !lower.ends_with(['a', 'e', 'i', 'o', 'u']) && {
        let before = lower.chars().rev().nth(1);
        before.is_some_and(|c| !"aeiou".contains(c))
    };
    if consonant_y {
        format!("{}ies", &noun[..noun.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|end| lower.ends_with(end)) {
        format!("{noun}es")
    } else {
        format!("{noun}s")
    }
}

/// `MyApp` becomes the app name and `myapp` its lowercase. The order matters: the app name is mixed case, so
/// the second pass cannot touch what the first produced.
pub fn replace_app_tokens(text: &str, app_name: &str, app_lower: &str) -> String {
    text.replace("MyApp", app_name).replace("myapp", app_lower)
}

/// Applies `(token, value)` pairs in order. Longer tokens must come before tokens they contain.
pub fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .fold(template.to_string(), |text, (token, value)| text.replace(token, value))
}

pub fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// Writes a file, creating its directory first, the way every generator lays down a new file.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_above_the_first_closing_item_group_with_its_indentation() {
        let csproj = "<Project>\n  <ItemGroup>\n    <A />\n  </ItemGroup>\n  <ItemGroup>\n  </ItemGroup>\n</Project>\n";
        let edited = insert_before_closing_item_group(csproj, "    <B />", "\n");
        assert_eq!(
            edited,
            "<Project>\n  <ItemGroup>\n    <A />\n    <B />\n  </ItemGroup>\n  <ItemGroup>\n  </ItemGroup>\n</Project>\n"
        );
    }

    #[test]
    fn hyphenates_pascal_case() {
        assert_eq!(hyphenate("OrderLine"), "order-line");
        assert_eq!(hyphenate("Product"), "product");
    }

    #[test]
    fn picks_the_article_by_sound() {
        assert_eq!(with_article("Invoice"), "an Invoice");
        assert_eq!(with_article("Order"), "an Order");
        assert_eq!(with_article("Product"), "a Product");
        assert_eq!(with_article("User"), "a User");
        assert_eq!(with_article("UnitPrice"), "a UnitPrice");
        assert_eq!(with_article("Umbrella"), "an Umbrella");
    }

    #[test]
    fn pluralizes_entity_names() {
        assert_eq!(plural("Invoice"), "Invoices");
        assert_eq!(plural("Category"), "Categories");
        assert_eq!(plural("Day"), "Days");
        assert_eq!(plural("Address"), "Addresses");
        assert_eq!(plural("Box"), "Boxes");
        assert_eq!(plural("Batch"), "Batches");
    }

    #[test]
    fn replaces_only_the_first_anchor() {
        assert_eq!(replace_first("a b a", "a", "c"), "c b a");
        assert_eq!(replace_first("x", "a", "c"), "x");
    }
}
