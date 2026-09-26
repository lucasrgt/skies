use std::path::PathBuf;

use super::*;

fn files() -> SpecFiles {
    SpecFiles {
        dir: ".specs/0001-toggle/e2e/".into(),
        files: vec!["toggle_test.dart".into(), "ToggleSpec.cs".into()],
    }
}

fn case(name: &str, file: Option<&str>, message: &str) -> Case {
    Case {
        name: name.into(),
        outcome: Outcome::Failed,
        message: Some(message.into()),
        file: file.map(String::from),
    }
}

#[test]
fn a_compile_error_in_the_specs_own_e2e_is_the_specs_doing() {
    let log = "  Determining projects to restore...\n\
               /w/.skies-red/red-x/checkout/.specs/0001-toggle/e2e/ToggleSpec.cs(12,9): error CS0246: The type \
               or namespace name 'Toggle' could not be found [/w/tests/App.Tests.csproj]\n\
               \u{1b}[31m/w/.specs/0001-toggle/e2e/ToggleSpec.cs(12,9): error CS0246: The type or namespace name \
               'Toggle' could not be found [/w/tests/App.Tests.csproj]\u{1b}[0m\n\
               Build FAILED.\n    0 Warning(s)\n    1 Error(s)\n";
    let lines = attribute(log, None, &files()).unwrap();
    assert_eq!(lines.len(), 2, "each located line once: {lines:?}");
    assert!(lines[0].contains("error CS0246"));

    let windows = r"C:\w\.specs\0001-toggle\e2e\ToggleSpec.cs(3,1): error CS1002: ; expected";
    assert!(attribute(windows, None, &files()).is_ok());
    let tsc = ".specs/0001-toggle/e2e/toggle.test.tsx:3:22 - error TS2307: Cannot find module './Toggle'";
    assert!(attribute(tsc, None, &files()).is_ok());
    let dart = "test/.skies_spec/toggle_test.dart:3:8: Error: Error when reading 'lib/toggle.dart': No such file";
    assert!(
        attribute(dart, None, &files()).is_ok(),
        "a runner's copy under .skies_spec/"
    );
    let analyzer = "error • Undefined name 'Toggle' • test/.skies_spec/toggle_test.dart:9:5 • undefined_identifier";
    assert!(attribute(analyzer, None, &files()).is_ok());
}

#[test]
fn a_failure_that_is_not_the_specs_aborts_with_the_reason() {
    let restore = "/w/tests/App.Tests/App.Tests.csproj : error NU1101: Unable to find package Skies.Framework.Testing.";
    let why = attribute(restore, None, &files()).unwrap_err();
    assert!(
        why.contains("outside the spec's e2e") && why.contains("NU1101"),
        "{why}"
    );

    let elsewhere = "/w/src/Api/Modules/Toggle/Toggle.cs(3,5): error CS0103: The name 'x' does not exist\n\
                     /w/.specs/0001-toggle/e2e/ToggleSpec.cs(12,9): error CS0246: 'Toggle' could not be found";
    assert!(
        attribute(elsewhere, None, &files())
            .unwrap_err()
            .contains("Toggle.cs(3,5)")
    );

    let unlocated = "MSBUILD : error MSB1009: Project file does not exist.";
    assert!(attribute(unlocated, None, &files()).is_err());
    assert!(attribute("error MSB1009: Project file does not exist.", None, &files()).is_err());
    let other_copy = "test/.skies_spec/other_test.dart:3:8: Error: Undefined name";
    assert!(
        attribute(other_copy, None, &files()).is_err(),
        "not a copy of one of the spec's files"
    );

    for nothing in [
        "sh: 1: dotnet: not found\n",
        "",
        "No test files found, exiting with code 1\n",
    ] {
        let why = attribute(nothing, None, &files()).unwrap_err();
        assert!(why.contains("nothing in its output points at the spec's e2e"), "{why}");
    }
}

#[test]
fn a_file_level_failure_counts_only_for_the_specs_own_files() {
    let vitest = case(
        "../examples/app/.specs/0001-toggle/e2e/toggle.test.tsx",
        Some("../examples/app/.specs/0001-toggle/e2e/toggle.test.tsx"),
        "Failed to resolve import \"./Toggle\" from \"../examples/app/.specs/0001-toggle/e2e/toggle.test.tsx\". Does the \
         file exist?",
    );
    let lines = attribute("", Some(&[vitest]), &files()).unwrap();
    assert!(lines[0].contains("Failed to resolve import"), "{lines:?}");

    let flutter = case(
        "loading /w/app/test/.skies_spec/toggle_test.dart",
        None,
        "Failed to load",
    );
    assert!(attribute("", Some(&[flutter]), &files()).is_ok());

    let deeper = case(
        ".specs/0001-toggle/e2e/toggle.test.tsx",
        None,
        "Failed to resolve import \"./missing\" from \"src/Toggle.tsx\". Does the file exist?",
    );
    let why = attribute("", Some(&[deeper]), &files()).unwrap_err();
    assert!(why.contains("src/Toggle.tsx, outside the spec's e2e"), "{why}");

    let helper = case("setup", Some("tests/setup.ts"), "boom");
    assert!(attribute("", Some(&[helper]), &files()).is_err());

    // The spec's file failed to load, but because the runner's setup file is missing in red's checkout.
    let setup = case(
        ".specs/0001-toggle/e2e/toggle.test.tsx",
        None,
        "Error: Cannot find module '/frontend-sdk/vitest.setup.ts'\nRequire stack: …",
    );
    let why = attribute("", Some(&[setup]), &files()).unwrap_err();
    assert!(
        why.contains("fails on /frontend-sdk/vitest.setup.ts, outside the spec's e2e"),
        "{why}"
    );

    let node = case(
        "/w/.specs/0001-toggle/e2e/toggle.test.ts",
        None,
        "Cannot find module '/w/src/Toggle.ts' imported from /w/.specs/0001-toggle/e2e/toggle.test.ts",
    );
    assert!(
        attribute("", Some(&[node]), &files()).is_ok(),
        "the spec itself imports what is missing"
    );

    let dart = case(
        "loading /w/app/test/.skies_spec/toggle_test.dart",
        None,
        "Failed to load: test/.skies_spec/toggle_test.dart:3:8: Error: Undefined name 'Toggle'.\n\
         lib/main.dart:9:1: Error: Expected ';'",
    );
    assert!(
        attribute("", Some(&[dart]), &files())
            .unwrap_err()
            .contains("lib/main.dart")
    );
}

#[test]
fn spec_files_are_read_from_the_e2e_folder() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(".specs/0001-toggle");
    std::fs::create_dir_all(path.join("e2e/nested")).unwrap();
    std::fs::write(path.join("e2e/nested/a_test.dart"), "").unwrap();
    let spec = SpecDir {
        name: "0001-toggle".into(),
        id: "0001".into(),
        path: PathBuf::from(&path),
    };
    let files = SpecFiles::new(&spec);
    assert!(files.owns("app/test/.skies_spec/nested/a_test.dart:1:1"));
    assert!(!files.owns("app/test/.skies_spec/nested/a_test.dart.bak"));
    assert!(files.owns("/abs/.specs/0001-toggle/e2e/anything.cs"));
    assert!(!files.owns("/abs/.specs/0002-other/e2e/anything.cs"));
}
