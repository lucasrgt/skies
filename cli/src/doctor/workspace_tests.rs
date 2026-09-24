use super::*;

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        if let Some(folder) = file.strip_suffix('/') {
            fs::create_dir_all(dir.path().join(folder)).unwrap();
            continue;
        }
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }
    dir
}

fn declared(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| entry.to_string()).collect()
}

fn codes(findings: &[Finding]) -> Vec<(String, String)> {
    findings
        .iter()
        .map(|f| (f.code.clone(), f.message.split_whitespace().next().unwrap().to_string()))
        .collect()
}

#[test]
fn only_what_git_would_see_counts() {
    let dir = tree(&[
        "Skies.toml",
        ".git/HEAD",
        ".gitignore",
        "src/App.cs",
        "bin/out.dll",
        "node_modules/x/index.js",
        ".claude/worktrees/a/file",
        "empty/",
        "debug.log",
    ]);
    fs::write(
        dir.path().join(".gitignore"),
        "bin/\nnode_modules/\n.claude/worktrees/\n*.log\n",
    )
    .unwrap();

    let names: Vec<String> = visible_entries(dir.path()).iter().map(Entry::display).collect();
    assert_eq!(names, ["src/", ".gitignore"]);
}

#[test]
fn undeclared_entries_are_errors_and_unmatched_declarations_are_warnings() {
    let dir = tree(&[
        "Skies.toml",
        ".gitignore",
        "src/App.cs",
        "tests/T.cs",
        "App.slnx",
        "notes.md",
        "screenshots/a.png",
        "docs",
    ]);
    let (status, findings) = check(
        dir.path(),
        Some(&declared(&[
            "src/",
            "tests/",
            "*.slnx",
            ".gitignore",
            "docs/",
            "README.md",
            "*.sln",
        ])),
    );
    assert_eq!(status, Status::Ran);
    assert_eq!(
        codes(&findings),
        [
            ("SKYWS001".into(), "screenshots/".into()),
            ("SKYWS001".into(), "docs".into()),
            ("SKYWS001".into(), "notes.md".into()),
            ("SKYWS002".into(), "`docs/`".into()),
            ("SKYWS002".into(), "`README.md`".into()),
            ("SKYWS002".into(), "`*.sln`".into()),
        ]
    );
    assert!(
        findings[1]
            .message
            .contains("declared as a folder; a file takes no trailing slash")
    );
    assert_eq!(findings[3].severity, Severity::Warning);
    assert_eq!(findings[0].file, dir.path().join("screenshots"));
}

#[test]
fn an_ignored_entry_may_be_declared_without_going_stale() {
    let dir = tree(&["Skies.toml", ".gitignore", "target/x"]);
    fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
    let (_, findings) = check(dir.path(), Some(&declared(&[".gitignore", "target/"])));
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn a_manifest_without_root_gets_one_finding_with_a_list_ready_to_paste() {
    let dir = tree(&["src/App.cs", "README.md", "junk [1].png"]);
    fs::write(dir.path().join("Skies.toml"), "# app\n[workspace]\nname = \"a\"\n").unwrap();

    let (_, findings) = check(dir.path(), None);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "SKYWS001");
    assert_eq!(findings[0].line, Some(2));
    let suggestion = findings[0].message.split_once(":\n").unwrap().1;
    let parsed: toml::Table = toml::from_str(&suggestion.replace("\n    ", "\n")).unwrap();
    let root: Vec<String> = parsed["root"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(root, ["src/", "README.md", "junk [[]1[]].png"]);
    let allowlist = Allowlist::compile(&root).unwrap();
    assert!(visible_entries(dir.path()).iter().all(|entry| allowlist.allows(entry)));
}

#[test]
fn escaped_patterns_match_only_their_own_odd_names() {
    let odd = ["\".skies", "a*b", "q?", "{x,y}", "[z]", "back\\slash", "!bang"];
    let dir = tree(
        &odd.map(|name| format!("{name}/f"))
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    fs::write(dir.path().join("a-b"), "").unwrap();
    let entries = visible_entries(dir.path());
    for entry in &entries {
        if !entry.is_dir {
            continue;
        }
        let allowlist = Allowlist::compile(&[entry.pattern()]).unwrap();
        let matched: Vec<&str> = entries
            .iter()
            .filter(|other| allowlist.allows(other))
            .map(|other| other.name.as_str())
            .collect();
        assert_eq!(matched, [entry.name.as_str()], "{}", entry.pattern());
    }
    let rendered = render_root(&entries.iter().map(Entry::pattern).collect::<Vec<_>>());
    let parsed: toml::Table = toml::from_str(&rendered).unwrap();
    assert_eq!(parsed["root"].as_array().unwrap().len(), entries.len());
}

#[test]
fn a_root_entry_that_is_a_path_or_a_bad_glob_stops_the_leg() {
    let dir = tree(&["Skies.toml"]);
    for bad in ["src/Api/", "/", "[oops"] {
        let (status, _) = check(dir.path(), Some(&declared(&[bad])));
        assert!(matches!(status, Status::Failed(_)), "{bad}");
    }
}
