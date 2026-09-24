use super::*;

fn spec_in(dir: &Path) -> SpecDir {
    let path = dir.join(".specs/0001-a");
    std::fs::create_dir_all(&path).unwrap();
    SpecDir {
        name: "0001-a".into(),
        id: "0001".into(),
        path,
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn files_under(dir: &Path) -> Vec<String> {
    walk(dir).unwrap().into_iter().map(|(rel, _)| rel).collect()
}

#[test]
fn halves_split_by_the_red_prefix() {
    assert!(Half::Red.owns("red.trx"));
    assert!(Half::Red.owns("raw/red.log"));
    assert!(Half::Red.owns("red.avp-FM-2.json"));
    assert!(Half::Green.owns("avp-FM-2.json"));
    assert!(Half::Green.owns("raw/green.trx"));
    assert!(Half::Green.owns("shots/final.png"));
    assert!(!Half::Green.owns("raw/red.trx"));
}

#[test]
fn the_ignore_rule_is_written_once_and_kept_with_the_team_rules() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join(".specs/.gitignore"), "/*/evidence/*.zip");
    ensure_ignored(dir.path()).unwrap();
    let first = std::fs::read_to_string(dir.path().join(".specs/.gitignore")).unwrap();
    assert!(first.starts_with("/*/evidence/*.zip\n"), "{first}");
    assert!(first.lines().any(|line| line == IGNORE_RULE), "{first}");
    ensure_ignored(dir.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".specs/.gitignore")).unwrap(),
        first
    );
}

#[test]
fn publishing_green_replaces_only_green_and_keeps_raw_local() {
    let dir = tempfile::tempdir().unwrap();
    let spec = spec_in(dir.path());
    let evidence = spec.file(EVIDENCE_DIR);
    write(&evidence.join("red.avp-FM-1.json"), "{}");
    write(&evidence.join("raw/red.trx"), "<TestRun/>");
    write(&evidence.join("old-green.png"), "stale");
    write(&evidence.join("raw/green.xml"), "old");

    let staged = dir.path().join("staged");
    write(&staged.join("final.txt"), "screenshot");
    write(&staged.join("raw/trace.zip"), "trace");
    let report = dir.path().join("report.xml");
    write(&report, "<testsuite/>");
    let files = Files {
        staged: Some(&staged),
        committed: Vec::new(),
        raw: vec![(report, "green.xml".into())],
    };
    publish(&spec, Half::Green, &files, &Scrub::new(&[])).unwrap();

    assert_eq!(
        files_under(&evidence),
        [
            "final.txt",
            "raw/green.xml",
            "raw/red.trx",
            "raw/trace.zip",
            "red.avp-FM-1.json"
        ]
    );
    let committed: Vec<String> = hash::evidence(&spec).unwrap().into_keys().collect();
    assert_eq!(
        committed,
        ["evidence/final.txt", "evidence/red.avp-FM-1.json"],
        "raw/ is never committed evidence"
    );
}

#[test]
fn a_verdict_that_differs_only_in_timings_keeps_its_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let spec = spec_in(dir.path());
    let old = r#"{"Results":[{"CriterionId":"k","Status":"Pass","durationMs":16.5}],"durationMs":16.6}"#;
    write(&spec.file("evidence/avp-FM-1.json"), old);

    let staged = dir.path().join("staged");
    write(
        &staged.join("avp-FM-1.json"),
        r#"{"Results":[{"CriterionId":"k","Status":"Pass","durationMs":19.1}],"durationMs":19.2}"#,
    );
    let files = Files {
        staged: Some(&staged),
        ..Files::default()
    };
    publish(&spec, Half::Green, &files, &Scrub::new(&[])).unwrap();
    assert_eq!(
        std::fs::read_to_string(spec.file("evidence/avp-FM-1.json")).unwrap(),
        old
    );

    write(
        &staged.join("avp-FM-1.json"),
        r#"{"Results":[{"CriterionId":"k","Status":"Fail","durationMs":19.1}]}"#,
    );
    publish(&spec, Half::Green, &files, &Scrub::new(&[])).unwrap();
    assert!(
        std::fs::read_to_string(spec.file("evidence/avp-FM-1.json"))
            .unwrap()
            .contains("Fail"),
        "a changed verdict is rewritten"
    );
}

#[test]
fn a_report_fingerprint_ignores_timings_but_not_results() {
    let trx = |start: &str, outcome: &str| {
        format!(
            r#"<TestRun><Times creation="{start}" start="{start}" finish="{start}" /><Results><UnitTestResult testName="FM-1: a" duration="00:00:00.{start}" startTime="{start}" endTime="{start}" outcome="{outcome}" /></Results></TestRun>"#
        )
    };
    assert_eq!(
        fingerprint(&trx("0223537", "Passed")),
        fingerprint(&trx("0547302", "Passed"))
    );
    assert_ne!(
        fingerprint(&trx("0223537", "Passed")),
        fingerprint(&trx("0223537", "Failed"))
    );
    let junit = |time: &str| {
        format!(r#"<testsuite timestamp="{time}" time="{time}"><testcase name="FM-1: a" time="{time}"/></testsuite>"#)
    };
    assert_eq!(fingerprint(&junit("0.29")), fingerprint(&junit("0.31")));
}

#[test]
fn the_half_not_rerun_keeps_its_recorded_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let spec = spec_in(dir.path());
    write(&spec.file("evidence/red.avp-FM-1.json"), "edited");
    write(&spec.file("evidence/avp-FM-1.json"), "fresh");
    let previous: Hashes = [
        ("evidence/red.avp-FM-1.json".to_string(), "blake3:recorded".to_string()),
        ("evidence/red.trx".to_string(), "blake3:moved".to_string()),
        ("evidence/avp-FM-1.json".to_string(), "blake3:old".to_string()),
    ]
    .into();
    let kept = hashes(&spec, Some(&previous), Some(Half::Red), &["evidence/red.trx".into()]).unwrap();
    assert_eq!(kept["evidence/red.avp-FM-1.json"], "blake3:recorded");
    assert!(!kept.contains_key("evidence/red.trx"));
    assert_ne!(kept["evidence/avp-FM-1.json"], "blake3:old");
}
