//! Dart name derivation: file stems are `snake_case`, types are `PascalCase`, both from one feature token.

/// `"UserWallets"` / `"user-wallets"` → `"user_wallets"`.
pub fn snake(value: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in value.trim().chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase()
                && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
            {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
        previous = Some(c);
    }
    out.trim_matches('_').to_string()
}

/// `"user-wallets"` → `"UserWallets"`.
pub fn pascal(value: &str) -> String {
    snake(value)
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part[..1].to_ascii_uppercase() + &part[1..])
        .collect()
}

/// A lowercase Dart package identifier (`pubName`), the shape pub and the generator both require.
pub fn is_package_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_idiomatic_dart_names() {
        assert_eq!(snake("UserWallets"), "user_wallets");
        assert_eq!(snake("  user-wallets "), "user_wallets");
        assert_eq!(snake("HTTPClient2Go"), "httpclient2_go");
        assert_eq!(pascal("user-wallets"), "UserWallets");
        assert_eq!(pascal("sample_api"), "SampleApi");
    }

    #[test]
    fn validates_package_names() {
        assert!(is_package_name("sample_api"));
        assert!(!is_package_name("Sample"));
        assert!(!is_package_name("sample,hide=true"));
        assert!(!is_package_name(""));
    }
}
