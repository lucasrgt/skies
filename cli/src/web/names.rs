//! Name derivation for React scaffolds.
//!
//! One feature name fans out into several spellings (component, hook, i18n namespace, entity). Deriving them in
//! one place keeps the emitted files agreeing with each other, which is what makes the unit typecheck.

/// `"user-profile"` / `"userProfiles"` → `"UserProfiles"`: split on anything that is not alphanumeric and
/// capitalize each word, leaving the rest of the word alone so existing camel humps survive.
pub fn pascal(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or_default();
            std::iter::once(first).chain(chars).collect::<String>()
        })
        .collect()
}

/// `"Bookings"` → `"bookings"`.
pub fn camel(value: &str) -> String {
    let pascal = pascal(value);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// A naive singular for the entity type (`bookings` → `booking`, `categories` → `category`). It is a scaffold
/// convenience the author refines, not an inflection library.
pub fn singular(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.ends_with("ies") {
        format!("{}y", &value[..value.len() - 3])
    } else if lower.ends_with('s') && !lower.ends_with("ss") {
        value[..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// A JavaScript-identifier-safe token for an i18n namespace (`user-profile` → `user_profile`).
pub fn ident(namespace: &str) -> String {
    namespace
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_the_spellings_the_scaffold_needs() {
        assert_eq!(pascal("user-profiles"), "UserProfiles");
        assert_eq!(pascal("userProfiles"), "UserProfiles");
        assert_eq!(camel("Bookings"), "bookings");
        assert_eq!(singular("bookings"), "booking");
        assert_eq!(singular("categories"), "category");
        assert_eq!(singular("address"), "address");
        assert_eq!(ident("user-profile"), "user_profile");
    }
}
