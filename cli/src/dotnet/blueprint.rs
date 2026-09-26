//! Rendering for the auth blueprints (`templates/dotnet/auth*` and their specs).
//!
//! Two passes that never touch each other's concerns:
//!
//! 1. **Flag regions.** Line markers `//<lz:tenancy>` … `//</lz:tenancy>` (and the `!` negations, likewise for
//!    `cookies`) keep or drop the enclosed lines; the marker lines always go. One template carries both shapes of a
//!    file and the generator picks one, so the variants cannot drift apart.
//! 2. **Tokens.** `MyApp` becomes the app name and `myapp` its lowercase, in contents and paths alike: namespaces,
//!    the cookie name, the JWT audience, and the dev secret all fall out of the same replace.

use super::text;

/// The two opt-out flags of `g auth`. Flows (otp/oauth/email) only exist for the default shape.
#[derive(Clone, Copy, Debug)]
pub struct Flags {
    pub tenancy: bool,
    pub cookies: bool,
}

impl Flags {
    pub const DEFAULT: Flags = Flags {
        tenancy: true,
        cookies: true,
    };
}

/// Resolves the flag regions, then replaces the app tokens.
pub fn render(body: &str, app_name: &str, app_lower: &str, flags: Flags) -> String {
    text::replace_app_tokens(&resolve_regions(body, flags), app_name, app_lower)
}

/// Strips `.cstmpl` and applies the token pass, so `Modules/Account/Slices/Login.cs.cstmpl` lands at
/// `Modules/Account/Slices/Login.cs`.
pub fn render_path(logical: &str, app_name: &str, app_lower: &str) -> String {
    let without_suffix = logical.strip_suffix(".cstmpl").unwrap_or(logical);
    text::replace_app_tokens(without_suffix, app_name, app_lower)
}

/// Walks the lines once, counting how many enclosing regions currently drop their contents. Once a region
/// drops, nested regions only deepen the count, whatever their own flag says.
fn resolve_regions(body: &str, flags: Flags) -> String {
    let normalized = text::normalize_newlines(body);
    let mut kept: Vec<&str> = Vec::new();
    let mut suppress = 0usize;
    for line in normalized.split('\n') {
        match Marker::parse(line) {
            Some(marker) if marker.open => {
                if suppress > 0 || !marker.allows(flags) {
                    suppress += 1;
                }
            }
            Some(_) => suppress = suppress.saturating_sub(1),
            None if suppress == 0 => kept.push(line),
            None => {}
        }
    }
    kept.join("\n")
}

struct Marker<'a> {
    open: bool,
    negate: bool,
    name: &'a str,
}

impl<'a> Marker<'a> {
    fn parse(line: &'a str) -> Option<Marker<'a>> {
        let trimmed = line.trim();
        let (open, inner) = match trimmed.strip_prefix("//<lz:") {
            Some(rest) => (true, rest),
            None => (false, trimmed.strip_prefix("//</lz:")?),
        };
        let inner = inner.trim_end_matches('>');
        let (negate, name) = match inner.strip_prefix('!') {
            Some(name) => (true, name),
            None => (false, inner),
        };
        Some(Marker { open, negate, name })
    }

    fn allows(&self, flags: Flags) -> bool {
        let on = match self.name {
            "tenancy" => flags.tenancy,
            "cookies" => flags.cookies,
            other => panic!("unknown flag region '{other}' in an auth template"),
        };
        on != self.negate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "a\n//<lz:tenancy>\nb\n    //<lz:!cookies>\nc\n    //</lz:!cookies>\n//</lz:tenancy>\n\
                        //<lz:!tenancy>\nd\n//</lz:!tenancy>\ne\n";

    #[test]
    fn keeps_the_regions_the_flags_allow_and_drops_every_marker() {
        let both = resolve_regions(BODY, Flags::DEFAULT);
        assert_eq!(both, "a\nb\ne\n");
        let no_cookies = resolve_regions(
            BODY,
            Flags {
                tenancy: true,
                cookies: false,
            },
        );
        assert_eq!(no_cookies, "a\nb\nc\ne\n");
        let single_tenant = resolve_regions(
            BODY,
            Flags {
                tenancy: false,
                cookies: false,
            },
        );
        assert_eq!(
            single_tenant, "a\nd\ne\n",
            "a nested region inside a dropped one stays dropped"
        );
    }

    #[test]
    fn paths_lose_the_suffix_and_gain_the_app_name() {
        assert_eq!(
            render_path("Tests/TestApp.cs.cstmpl", "Acme", "acme"),
            "Tests/TestApp.cs"
        );
        assert_eq!(
            render("namespace MyApp.Api; // myapp", "Acme", "acme", Flags::DEFAULT),
            "namespace Acme.Api; // acme"
        );
    }
}
