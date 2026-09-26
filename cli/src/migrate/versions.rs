//! Moves package references onto the release this binary belongs to. Every Skies package ships in lockstep, so the
//! binary's own version is the one the NuGet and pub references must name; a 4.x analyzer or runtime next to the
//! 5.x tooling would bring back the removed proof rules.

use std::sync::LazyLock;

use regex::Regex;

use super::Plan;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A `PackageReference`/`PackageVersion` of `Skies` or `Skies.Framework*` on one line.
static NUGET_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<Package(?:Reference|Version)\s+(?:Include|Update)="(?:Skies|Skies\.Framework(?:\.[A-Za-z0-9.]+)?)""#)
        .unwrap()
});

static VERSION_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(Version|VersionOverride)="([^"$]*)""#).unwrap());

/// Bumps the Skies NuGet references in a project, props, or targets file.
pub fn nuget(text: &str, _: &str, _: &mut Plan) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if !NUGET_REFERENCE.is_match(line) {
            out.push_str(line);
            continue;
        }
        let updated = VERSION_ATTRIBUTE.replace_all(line, |captures: &regex::Captures| {
            format!("{}=\"{VERSION}\"", &captures[1])
        });
        changed |= updated != line;
        out.push_str(&updated);
    }
    changed.then_some(out)
}

static PUB_DEPENDENCY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s+)skies_flutter:[ \t]*([^#\s]*)(.*)$").unwrap());

/// Bumps a hosted `skies_flutter` dependency. Path and git dependencies point at a checkout the app controls, so
/// they are left alone.
pub fn pubspec(text: &str, _: &str, plan: &mut Plan) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let body = line.trim_end_matches(['\n', '\r']);
        let Some(captures) = PUB_DEPENDENCY.captures(body) else {
            out.push_str(line);
            index += 1;
            continue;
        };
        let (indent, value, rest) = (&captures[1], &captures[2], &captures[3]);
        let ending = &line[body.len()..];
        if !value.is_empty() {
            let updated = format!("{indent}skies_flutter: {}{rest}{ending}", pinned(value));
            changed |= updated != line;
            out.push_str(&updated);
            index += 1;
            continue;
        }
        // The map form: `version:` under it is a hosted constraint; `path:`/`git:` mean a local or pinned source.
        out.push_str(line);
        index += 1;
        let child_indent = indent.len();
        let start = index;
        while index < lines.len() && is_child(lines[index], child_indent) {
            index += 1;
        }
        let children = &lines[start..index];
        let sourced = children.iter().any(|child| {
            let key = child.trim_start();
            key.starts_with("path:") || key.starts_with("git:")
        });
        for child in children {
            let trimmed = child.trim_start();
            match trimmed.strip_prefix("version:") {
                Some(version) if !sourced => {
                    let child_body = child.trim_end_matches(['\n', '\r']);
                    let pad = &child[..child.len() - trimmed.len()];
                    let updated = format!("{pad}version: {}{}", pinned(version.trim()), &child[child_body.len()..]);
                    changed |= updated != *child;
                    out.push_str(&updated);
                }
                _ => out.push_str(child),
            }
        }
    }
    if changed {
        plan.follow_up("run `flutter pub get` in each Flutter package to refresh pubspec.lock");
    }
    changed.then_some(out)
}

fn is_child(line: &str, parent_indent: usize) -> bool {
    let trimmed = line.trim_start();
    !trimmed.is_empty() && line.len() - trimmed.len() > parent_indent
}

/// The 5.x version, keeping a caret and any quoting the author used.
fn pinned(value: &str) -> String {
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
    let bare = value.trim_matches(['"', '\'']);
    let operator = if bare.starts_with('^') { "^" } else { "" };
    match quote {
        Some(quote) => format!("{quote}{operator}{VERSION}{quote}"),
        None => format!("{operator}{VERSION}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumps_skies_nuget_references_only() {
        let text = "    <PackageReference Include=\"Skies.Framework\" Version=\"4.1.4\" />\n    <PackageReference Include=\"Skies.Framework.Doctor\" Version=\"4.1.0\" PrivateAssets=\"all\" />\n    <PackageVersion Include=\"Skies\" Version=\"4.0.0\" />\n    <PackageReference Include=\"SkiesUnrelated\" Version=\"1.0.0\" />\n    <PackageReference Include=\"Skies.Framework.Testing\" Version=\"$(SkiesVersion)\" />\n";
        let out = nuget(text, "a.csproj", &mut Plan::default()).unwrap();
        assert_eq!(
            out,
            format!(
                "    <PackageReference Include=\"Skies.Framework\" Version=\"{VERSION}\" />\n    <PackageReference Include=\"Skies.Framework.Doctor\" Version=\"{VERSION}\" PrivateAssets=\"all\" />\n    <PackageVersion Include=\"Skies\" Version=\"{VERSION}\" />\n    <PackageReference Include=\"SkiesUnrelated\" Version=\"1.0.0\" />\n    <PackageReference Include=\"Skies.Framework.Testing\" Version=\"$(SkiesVersion)\" />\n"
            )
        );
        assert!(nuget(&out, "a.csproj", &mut Plan::default()).is_none());
    }

    #[test]
    fn bumps_hosted_skies_flutter_and_leaves_local_sources() {
        let text = "dependencies:\n  skies_flutter: 4.1.22\n  other: ^1.0.0\ndev_dependencies:\n  skies_flutter:\n    path: ../skies/flutter-sdk/packages/skies_flutter\n";
        let mut plan = Plan::default();
        let out = pubspec(text, "pubspec.yaml", &mut plan).unwrap();
        assert_eq!(
            out,
            format!(
                "dependencies:\n  skies_flutter: {VERSION}\n  other: ^1.0.0\ndev_dependencies:\n  skies_flutter:\n    path: ../skies/flutter-sdk/packages/skies_flutter\n"
            )
        );
        let hosted = "dependencies:\n  skies_flutter:\n    hosted: https://pub.dev\n    version: ^4.1.0\n";
        assert_eq!(
            pubspec(hosted, "pubspec.yaml", &mut plan).unwrap(),
            format!("dependencies:\n  skies_flutter:\n    hosted: https://pub.dev\n    version: ^{VERSION}\n")
        );
        assert!(pubspec(&out, "pubspec.yaml", &mut plan).is_none());
    }
}
