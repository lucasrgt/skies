//! Per-file edits: strip Skies 4 proof ceremony from source, project files, hooks, and agent instructions, and
//! route package manifests, lint configs, and CI workflows to the modules that migrate them.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::Plan;

/// Attributes that existed only for the Skies 4 gate. `AVP` is Assay.Net's; the rest were Skies.Framework.Testing's.
const CEREMONY_ATTRIBUTES: &[&str] = &[
    "AVP",
    "Assay.Net.AVP",
    "Journey",
    "Unit",
    "Integration",
    "E2E",
    "Skies.Framework.Testing.Journey",
];

/// JSDoc and Dart doc tags that bound code to AVP criteria and E2E flows.
static DOC_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*((\*|///|//)\s*@(verify|avp|e2e|backendSlice|skies-criterion|skies-proof)\b|/\*\*\s*@(verify|avp|e2e|backendSlice|skies-criterion|skies-proof)\b.*\*/\s*$)",
    )
    .unwrap()
});

/// `flows.json` inside a string literal: code that still reads or writes the removed flow contract. Comments that
/// merely mention it are not code.
static FLOWS_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"["'`][^"'`\n]*flows\.json["'`]"#).unwrap());

/// An attribute-only C# line such as `[Unit, Fact]` or `[Journey(typeof(Pay), JourneyPath.Happy)]`.
static ATTRIBUTE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)\[(.*)\]\s*$").unwrap());

/// A per-file-kind edit: the text and its root-relative path in, the new text out when anything changed.
type Edit = fn(&str, &str, &mut Plan) -> Option<String>;

pub fn migrate_file(root: &Path, path: &Path, plan: &mut Plan) -> Result<()> {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    let relative = path.strip_prefix(root).unwrap_or(path).display().to_string();

    if name.ends_with(".spec.toml") {
        let text = std::fs::read_to_string(path)?;
        if text.contains("[slices.") || text.contains("module =") {
            plan.delete(path);
        }
        return Ok(());
    }
    if name == "flows.json" && path.parent().is_some_and(|dir| dir.ends_with("e2e")) {
        plan.delete(path);
        return Ok(());
    }

    let in_workflows = path.parent().is_some_and(|dir| dir.ends_with(".github/workflows"));
    let edit: Option<Edit> = match (name, extension) {
        ("package.json", _) => Some(super::package_json::migrate),
        ("pubspec.yaml", _) => Some(super::versions::pubspec),
        (name, _) if super::eslint::is_config(name) => Some(super::eslint::migrate),
        (_, "yml" | "yaml") if in_workflows => Some(super::workflows::migrate),
        (_, "props" | "targets") => Some(super::versions::nuget),
        (_, "cs") => Some(csharp),
        (_, "csproj") => Some(project),
        (_, "ts" | "tsx" | "js" | "mjs" | "cjs" | "mts" | "cts" | "dart") => Some(doc_tags),
        ("AGENTS.md" | "CLAUDE.md" | "GEMINI.md", _) => Some(agent_instructions),
        ("lefthook.yml" | "lefthook.yaml", _) => Some(lefthook),
        ("dotnet-tools.json", _) => Some(dotnet_tools),
        _ => None,
    };
    let Some(edit) = edit else { return Ok(()) };
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(());
    };
    // A source file saved in a legacy 8-bit encoding still needs its imports migrated. Reading it byte for byte as
    // Latin-1 and writing it back the same way leaves every byte the migration did not touch exactly as it was.
    let (original, latin1) = match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(error) => (error.into_bytes().into_iter().map(char::from).collect(), true),
    };
    let vendored = if matches!(extension, "ts" | "tsx" | "js" | "mjs" | "cjs" | "mts" | "cts" | "dart") {
        super::vendor::rewrite(root, path, &original, plan)
    } else {
        None
    };
    let text = vendored.clone().unwrap_or_else(|| original.clone());
    if let Some(updated) = edit(&text, &relative, plan).or(vendored) {
        if updated.trim().is_empty() && name == "dotnet-tools.json" {
            plan.delete(path);
        } else if updated != original && !latin1 {
            plan.write(path, updated);
        } else if updated != original {
            match updated.chars().map(u8::try_from).collect::<Result<Vec<u8>, _>>() {
                Ok(encoded) => plan.write_bytes(path, encoded),
                Err(_) => plan.follow_up_file("is not UTF-8 and could not be edited; migrate it by hand", &relative),
            }
        }
    }
    Ok(())
}

/// Removes ceremony attributes, keeping any other attribute that shared the list (`[Unit, Fact]` → `[Fact]`).
fn csharp(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let Some(captures) = ATTRIBUTE_LINE.captures(body) else {
            out.push_str(line);
            continue;
        };
        let items = split_top_level(&captures[2]);
        let kept: Vec<&str> = items.iter().copied().filter(|item| !is_ceremony(item)).collect();
        if kept.len() == items.len() {
            out.push_str(line);
            continue;
        }
        changed = true;
        if !kept.is_empty() {
            out.push_str(&format!("{}[{}]{}", &captures[1], kept.join(", "), &line[body.len()..]));
        }
    }
    if changed {
        // `[Journey(typeof(Pay))]` or `[AVP]` was often the only user of a using; IDE0005 then warns on it.
        plan.follow_up(
            "run `dotnet format --diagnostics IDE0005` to drop usings only the removed attributes needed \
             (e.g. `using Assay.Net;`)",
        );
    }
    if text.contains("using Assay.Net") {
        plan.follow_up_file("uses Assay.Net; keep the package or rewrite the test", relative);
    }
    if text.contains("JourneyPath") && !changed {
        plan.follow_up_file("mentions JourneyPath outside an attribute", relative);
    }
    changed.then_some(out)
}

fn is_ceremony(item: &str) -> bool {
    let name = item.split('(').next().unwrap_or(item).trim();
    let name = name.strip_suffix("Attribute").unwrap_or(name);
    CEREMONY_ATTRIBUTES.contains(&name)
}

/// Splits an attribute list on commas that are not inside parentheses or strings.
fn split_top_level(list: &str) -> Vec<&str> {
    let (mut depth, mut in_string, mut start) = (0i32, false, 0);
    let mut items = Vec::new();
    for (index, character) in list.char_indices() {
        match character {
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                items.push(list[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(list[start..].trim());
    items.into_iter().filter(|item| !item.is_empty()).collect()
}

/// A project file gets both the spec-include edits and the Skies package bump.
fn project(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let edited = csproj(text, relative, plan);
    let base = edited.as_deref().unwrap_or(text);
    super::versions::nuget(base, relative, plan).or(edited)
}

/// Drops the spec-manifest AdditionalFiles include (and a comment right above it that only explained it), and makes
/// a test project that globs co-located `*.Tests.cs` also compile the spec E2E under `.specs/`.
fn csproj(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    if text.contains("Assay.Net") {
        plan.follow_up_file("references Assay.Net; remove it once no test uses it", relative);
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    for (index, line) in lines.iter().enumerate() {
        if line.contains("*.spec.toml") && line.contains("AdditionalFiles") {
            continue;
        }
        let explains_next = lines.get(index + 1).is_some_and(|next| next.contains("*.spec.toml"));
        if explains_next && line.trim_start().starts_with("<!--") && line.trim_end().ends_with("-->") {
            continue;
        }
        out.push_str(line);
        let globs_tests = line.contains("<Compile Include=") && line.contains("*.Tests.cs");
        if globs_tests && !text.contains(".specs") {
            let indent = &line[..line.len() - line.trim_start().len()];
            let depth = Path::new(relative).components().count().saturating_sub(1);
            let up = "..\\".repeat(depth);
            out.push_str(&format!(
                "{indent}<Compile Include=\"{up}.specs\\*\\e2e\\**\\*.cs\" />\n"
            ));
        }
    }
    (out != text).then_some(out)
}

/// An Assay proof keeps its tags; only its imports of removed Skies helpers are rewritten (by the vendor step).
fn assay_proof(_: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    plan.follow_up_file(
        "is an Assay proof; keep avp-assay or rewrite it as a spec E2E",
        relative,
    );
    None
}

/// Removes `@verify`/`@avp`/`@e2e` doc-comment lines, then any doc comment left empty.
fn doc_tags(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    if text.contains("defineVerification") {
        plan.follow_up_file(
            "is an Assay proof; keep avp-assay or rewrite it as a spec E2E",
            relative,
        );
    }
    if FLOWS_PATH.is_match(text) {
        plan.follow_up_file(
            "reads or writes e2e/flows.json, which Skies 5 deletes; retire the script or point it elsewhere",
            relative,
        );
    }
    if !text.contains('@') || !text.lines().any(|line| DOC_TAG.is_match(line)) {
        return None;
    }
    let kept: Vec<&str> = text
        .split_inclusive('\n')
        .filter(|line| !DOC_TAG.is_match(line))
        .collect();
    Some(drop_empty_doc_blocks(&kept.concat()))
}

fn drop_empty_doc_blocks(text: &str) -> String {
    static EMPTY_BLOCK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^[ \t]*/\*\*[ \t]*\r?\n([ \t]*\*[ \t]*\r?\n)*[ \t]*\*/[ \t]*\r?\n").unwrap());
    EMPTY_BLOCK.replace_all(text, "").into_owned()
}

/// Removes the generated `skies:foundations` block that told agents to run the gate and the CSM tools.
/// Hand-written sections that still send agents to the gate are prose, so they are reported rather than rewritten.
fn agent_instructions(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let updated = without_foundations(text);
    let remaining = updated.as_deref().unwrap_or(text);
    if ["skies check", "skies gate", "dotnet tool run skies"]
        .iter()
        .any(|command| remaining.contains(command))
    {
        plan.follow_up_file(
            "still tells agents to run the Skies 4 gate (`skies check`/`skies gate`); rewrite those sections",
            relative,
        );
    }
    updated
}

fn without_foundations(text: &str) -> Option<String> {
    let start = text.find("<!-- skies:foundations:start -->")?;
    let end_marker = "<!-- skies:foundations:end -->";
    let end = text[start..].find(end_marker)? + start + end_marker.len();
    let mut out = format!("{}{}", text[..start].trim_end(), &text[end..]);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

/// Drops hook commands that ran the Skies gate; anything else in the hooks is the application's and stays.
fn lefthook(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let runs_gate = |line: &str| {
        line.contains("run:")
            && ["skies check", "skies gate", "skies context"]
                .iter()
                .any(|cmd| line.contains(cmd))
    };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if !lines.iter().any(|line| runs_gate(line)) {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut commands_indent: Option<usize> = None;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let indent = indent_of(line);
        if !line.trim().is_empty() && commands_indent.is_some_and(|parent| indent <= parent) {
            commands_indent = None;
        }
        if line.trim() == "commands:" {
            commands_indent = Some(indent);
        } else if commands_indent.is_some() && line.trim_end().ends_with(':') {
            // A command under `commands:` owns every following line that is blank or indented deeper.
            let mut end = index + 1;
            while end < lines.len() && (lines[end].trim().is_empty() || indent_of(lines[end]) > indent) {
                end += 1;
            }
            // Blank lines after the command separate it from what follows; they stay when the command goes.
            while end > index + 1 && lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            if lines[index..end].iter().any(|line| runs_gate(line)) {
                index = end;
                continue;
            }
        }
        out.push_str(line);
        index += 1;
    }
    plan.follow_up_file(
        "removed the Skies gate commands; review what remains in each hook",
        relative,
    );
    Some(out)
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Removes the `skies-framework-cli` dotnet tool; the whole manifest goes if nothing else is left in it.
pub(super) fn dotnet_tools(text: &str, _: &str, _: &mut Plan) -> Option<String> {
    let mut json: serde_json::Value = serde_json::from_str(text).ok()?;
    let tools = json.get_mut("tools")?.as_object_mut()?;
    tools.remove("skies-framework-cli")?;
    if tools.is_empty() {
        return Some(String::new());
    }
    Some(serde_json::to_string_pretty(&json).ok()? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_latin_1_file_is_edited_without_touching_its_other_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.test.ts");
        let mut bytes = b"import { useSession } from \"skies-react\";\n// sess\xe3o\n".to_vec();
        std::fs::write(&path, &bytes).unwrap();
        let mut plan = Plan::default();

        migrate_file(dir.path(), &path, &mut plan).unwrap();

        bytes.splice(28..39, b"@skiesjs/react".iter().copied());
        assert_eq!(
            plan.changes,
            [super::super::Change::Write {
                path: path.clone(),
                content: bytes
            }]
        );
    }

    #[test]
    fn keeps_non_ceremony_attributes() {
        let mut plan = Plan::default();
        let text = "    [Unit, Fact]\n    [Theory, Integration, InlineData(\"a, b\")]\n    [Assay.Net.AVP(typeof(X), \"id\")]\n    [Migration(\"x\")]\n";
        let out = csharp(text, "a.cs", &mut plan).unwrap();
        assert_eq!(
            out,
            "    [Fact]\n    [Theory, InlineData(\"a, b\")]\n    [Migration(\"x\")]\n"
        );
    }

    #[test]
    fn test_projects_compile_spec_e2e() {
        let mut plan = Plan::default();
        let text = "    <Compile Include=\"..\\..\\src\\App.Api\\**\\*.Tests.cs\" />\n";
        let out = csproj(text, "tests/App.Tests/App.Tests.csproj", &mut plan).unwrap();
        assert_eq!(
            out,
            "    <Compile Include=\"..\\..\\src\\App.Api\\**\\*.Tests.cs\" />\n    <Compile Include=\"..\\..\\.specs\\*\\e2e\\**\\*.cs\" />\n"
        );
    }

    #[test]
    fn strips_dart_and_jsdoc_tags() {
        let mut plan = Plan::default();
        let dart = "/// Shows the host dashboard.\n/// @verify host-sees-own-bookings\n/// @e2e dashboard-happy\nclass DashboardViewModel {}\n";
        assert_eq!(
            doc_tags(dart, "a.dart", &mut plan).unwrap(),
            "/// Shows the host dashboard.\nclass DashboardViewModel {}\n"
        );
        let one_line =
            "/** @verify maps-role */\n/** @e2e choose-role-happy */\n/** Keeps this. */\nexport const b = 2;\n";
        assert_eq!(
            doc_tags(one_line, "b.ts", &mut plan).unwrap(),
            "/** Keeps this. */\nexport const b = 2;\n"
        );
        let ts = "/**\n * @verify x\n */\nexport const a = 1;\n";
        assert_eq!(doc_tags(ts, "a.ts", &mut plan).unwrap(), "export const a = 1;\n");
    }

    #[test]
    fn removes_only_gate_hooks() {
        let mut plan = Plan::default();
        let text = "pre-commit:\n  commands:\n    design-check:\n      run: npm run design:doctor\n    review:\n      run: dotnet tool run skies check --staged\n";
        assert_eq!(
            lefthook(text, "lefthook.yml", &mut plan).unwrap(),
            "pre-commit:\n  commands:\n    design-check:\n      run: npm run design:doctor\n"
        );
    }

    #[test]
    fn removes_the_foundations_block() {
        let mut plan = Plan::default();
        let text =
            "# App\n\nRules.\n\n<!-- skies:foundations:start -->\nRun skies check.\n<!-- skies:foundations:end -->\n";
        assert_eq!(
            agent_instructions(text, "AGENTS.md", &mut plan).unwrap(),
            "# App\n\nRules.\n"
        );
    }

    #[test]
    fn removes_the_skies_dotnet_tool() {
        let mut plan = Plan::default();
        let text = r#"{"version":1,"isRoot":true,"tools":{"dotnet-ef":{"version":"10.0.0","commands":["dotnet-ef"]},"skies-framework-cli":{"version":"4.1.15","commands":["skies"]}}}"#;
        let out = dotnet_tools(text, "", &mut plan).unwrap();
        assert!(out.contains("dotnet-ef") && !out.contains("skies-framework-cli"));
    }
}
