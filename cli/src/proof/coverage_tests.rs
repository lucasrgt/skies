use super::*;

const COBERTURA: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<coverage line-rate="0.5" version="1.9">
  <sources>
    <source>/repo/</source>
  </sources>
  <packages>
    <package name="App">
      <classes>
        <class name="App.Wallet" filename="app/src/Wallet.cs">
          <methods><method name="Deposit"><lines><line number="9" hits="3" /></lines></method></methods>
          <lines><line number="9" hits="3" /><line number="10" hits="0" /></lines>
        </class>
        <class name="App.Unused" filename="app/src/Unused.cs">
          <lines><line number="4" hits="0" /></lines>
        </class>
        <class name="App.Wallet/Nested" filename="app/src/Wallet.cs">
          <lines><line number="30" hits="0" /></lines>
        </class>
        <class name="App.Partial" filename="app/src/Platform.cs">
          <lines><line number="1" hits="0" /></lines>
        </class>
        <class name="App.Partial" filename="app/src/Platform.cs">
          <lines><line number="7" hits="12" /></lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>
"#;

const LCOV: &str = "TN:\nSF:lib/wallet.dart\nDA:1,1\nDA:2,0\nLF:2\nLH:1\nend_of_record\nSF:lib/unused.dart\nDA:1,0\nLF:1\nLH:0\nend_of_record\nSF:/abs/src/summary.ts\nLF:3\nLH:2\nend_of_record\n";

fn names(parsed: &Parsed) -> Vec<&str> {
    parsed.files.keys().map(String::as_str).collect()
}

fn lines(parsed: &Parsed, file: &str) -> Vec<u32> {
    parsed.files[file].iter().copied().collect()
}

#[test]
fn cobertura_keeps_files_with_an_executed_line_across_classes() {
    let parsed = parse(COBERTURA).unwrap().unwrap();
    assert_eq!(parsed.sources, ["/repo/"]);
    assert_eq!(names(&parsed), ["app/src/Platform.cs", "app/src/Wallet.cs"]);
    assert_eq!(lines(&parsed, "app/src/Wallet.cs"), [9], "only the lines with hits");
    assert_eq!(
        lines(&parsed, "app/src/Platform.cs"),
        [7],
        "the union over partial classes"
    );
}

#[test]
fn lcov_keeps_records_with_a_hit() {
    let parsed = parse(LCOV).unwrap().unwrap();
    assert!(parsed.sources.is_empty());
    assert_eq!(names(&parsed), ["/abs/src/summary.ts", "lib/wallet.dart"]);
    assert_eq!(lines(&parsed, "lib/wallet.dart"), [1]);
    assert!(
        lines(&parsed, "/abs/src/summary.ts").is_empty(),
        "a summary-only record is executed with no line data"
    );
}

#[test]
fn a_file_without_line_data_in_any_report_stays_without() {
    let mut covered = Covered::new();
    merge(&mut covered, "a".into(), &[1, 2].into());
    merge(&mut covered, "a".into(), &[5].into());
    assert_eq!(covered["a"], [1, 2, 5].into());
    merge(&mut covered, "a".into(), &BTreeSet::new());
    assert!(covered["a"].is_empty());
    merge(&mut covered, "a".into(), &[7].into());
    assert!(covered["a"].is_empty());
}

#[test]
fn other_files_are_not_coverage() {
    assert!(parse("<TestRun><Results/></TestRun>").unwrap().is_none());
    assert!(parse("<testsuites></testsuites>").unwrap().is_none());
    assert!(parse("plain text\n").unwrap().is_none());
    assert!(
        parse("<coverage><unclosed>").is_err(),
        "a broken Cobertura file is an error, not silence"
    );
}

#[test]
fn project_paths_come_from_absolute_names_under_any_checkout() {
    let roots = Roots::new(&[Path::new("/repo/app"), Path::new("/tmp/skies-red/app")]);
    assert_eq!(
        roots.relative(Path::new("/repo/app/src/A.cs")).as_deref(),
        Some("src/A.cs")
    );
    assert_eq!(
        roots.relative(Path::new("/tmp/skies-red/app/src/A.cs")).as_deref(),
        Some("src/A.cs"),
        "an absolute path in the red worktree maps back to the same project path"
    );
    assert_eq!(
        roots.relative(Path::new("/repo/app/src/../lib/B.cs")).as_deref(),
        Some("lib/B.cs")
    );
    assert_eq!(
        roots.relative(Path::new("/repo/src/Framework/X.cs")),
        None,
        "outside the project"
    );
    assert_eq!(
        roots.relative(Path::new("/repo/application/X.cs")),
        None,
        "a sibling that shares a prefix"
    );
    assert_eq!(roots.relative(Path::new("/repo/app")), None);
}

#[test]
fn relative_names_resolve_against_sources_then_the_run_then_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("mobile/lib")).unwrap();
    std::fs::create_dir_all(root.join("mobile/coverage")).unwrap();
    std::fs::write(root.join("mobile/lib/wallet.dart"), "").unwrap();
    let report = root.join("mobile/coverage/lcov.info");
    let mut dirs: Vec<&Path> = vec![root];
    dirs.extend(report.ancestors().skip(1));

    assert_eq!(
        resolve("lib/wallet.dart", &[], &dirs),
        root.join("mobile/lib/wallet.dart"),
        "Flutter writes names relative to the package, which holds coverage/"
    );
    let source = format!("{}/", root.display());
    assert_eq!(
        resolve("mobile/lib/wallet.dart", std::slice::from_ref(&source), &dirs),
        root.join("mobile/lib/wallet.dart")
    );
    assert_eq!(resolve("C:\\app\\A.cs", &[], &dirs), PathBuf::from("C:/app/A.cs"));
    assert_eq!(
        resolve("gone.cs", &[], &dirs),
        root.join("gone.cs"),
        "the first candidate when none exists"
    );
}

#[test]
fn build_output_generated_code_and_specs_are_not_sources() {
    assert!(is_source("backend/Api/Platform.cs"));
    assert!(is_source("lib/bin_utils.dart"));
    assert!(!is_source("backend/Api/obj/Debug/net10.0/OpenApi.generated.cs"));
    assert!(!is_source("backend/Api/bin/Debug/App.cs"));
    assert!(!is_source("backend/Api/Json.g.cs"));
    assert!(!is_source("lib/model.freezed.dart"));
    assert!(!is_source("web/src/client.gen/sdk.ts"));
    assert!(!is_source("web/node_modules/react/index.js"));
    assert!(!is_source(".specs/0001-a/e2e/Deposit.cs"));
}

#[test]
fn collect_reads_a_nested_coverlet_folder_and_maps_it_onto_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path();
    let root = top.join("app");
    for file in ["src/Wallet.cs", "src/Platform.cs"] {
        std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
        std::fs::write(root.join(file), "").unwrap();
    }
    let results = top.join("results");
    std::fs::create_dir_all(results.join("5b1e/In")).unwrap();
    let report = COBERTURA.replace("/repo/", &format!("{}/", top.display()));
    std::fs::write(results.join("5b1e/coverage.cobertura.xml"), &report).unwrap();
    std::fs::write(results.join("5b1e/In/run.trx"), "<TestRun/>").unwrap();
    let location = Location {
        path: results,
        configured: true,
    };

    let Outcome::Covered(covered) = collect(&location, "api", &root, &[&root]).unwrap() else {
        panic!("coverage expected");
    };
    assert_eq!(covered.keys().collect::<Vec<_>>(), ["src/Platform.cs", "src/Wallet.cs"]);
    assert_eq!(covered["src/Wallet.cs"], [9].into());
}

#[test]
fn collect_resolves_names_written_relative_to_a_directory_above_the_project() {
    // vitest run with a repository-root config names files from the repository root, above the Skies project.
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let project = repo.join("examples/app");
    std::fs::create_dir_all(project.join("web/src")).unwrap();
    std::fs::write(project.join("web/src/Pay.viewModel.ts"), "").unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let report = scratch.path().join("lcov.info");
    std::fs::write(
        &report,
        "SF:examples/app/web/src/Pay.viewModel.ts\nDA:1,3\nend_of_record\n",
    )
    .unwrap();
    let location = Location {
        path: report,
        configured: true,
    };

    let Outcome::Covered(covered) = collect(&location, "web", &project, &[&project]).unwrap() else {
        panic!("covered");
    };

    assert_eq!(covered.keys().collect::<Vec<_>>(), ["web/src/Pay.viewModel.ts"]);
}

#[test]
fn collect_explains_missing_coverage() {
    let dir = tempfile::tempdir().unwrap();
    let absent = |configured| Location {
        path: dir.path().join("none"),
        configured,
    };
    let Outcome::Missing(why) = collect(&absent(false), "api", dir.path(), &[dir.path()]).unwrap() else {
        panic!()
    };
    assert!(why.contains("writes no coverage"), "{why}");
    let Outcome::Missing(why) = collect(&absent(true), "api", dir.path(), &[dir.path()]).unwrap() else {
        panic!()
    };
    assert!(why.contains("wrote no coverage at"), "{why}");
}
