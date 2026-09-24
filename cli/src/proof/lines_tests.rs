use super::*;

fn set(lines: &[u32]) -> BTreeSet<u32> {
    lines.iter().copied().collect()
}

fn content(text: &str) -> Content {
    Content::new(text.as_bytes().to_vec())
}

/// Six lines; a spec that executes lines 2 and 4-5 (the handler body) but not 1, 3, or 6.
const FILE: &str = "class Deposit\n  Map();\n{\n  var total = a + b;\n  return total;\n}\n";

fn recorded() -> Print {
    print(Some(&content(FILE)), Some(&set(&[2, 4, 5])))
}

#[test]
fn ranges_compress_runs_and_singletons() {
    let ranges = Ranges::from_set(&set(&[12, 13, 14, 15, 16, 17, 18, 40, 55, 56, 57, 58, 59, 60])).unwrap();
    assert_eq!(ranges.to_string(), "12-18,40,55-60");
    assert_eq!(ranges.last(), 60);
    assert_eq!(ranges.iter().count(), 14);
    assert_eq!(Ranges::from_set(&set(&[3])).unwrap().to_string(), "3");
    assert_eq!(Ranges::from_set(&set(&[1, 3, 5])).unwrap().to_string(), "1,3,5");
    assert_eq!(
        Ranges::from_set(&set(&[0, 1, 2])).unwrap().to_string(),
        "1-2",
        "line 0 does not exist"
    );
    assert!(Ranges::from_set(&set(&[])).is_none());
    assert!(Ranges::from_set(&set(&[0])).is_none());
}

#[test]
fn ranges_parse_back_what_they_write_and_nothing_else() {
    for text in ["12-18,40,55-60", "1", "1,3,5", "7-8"] {
        assert_eq!(text.parse::<Ranges>().unwrap().to_string(), text);
    }
    let parsed: Ranges = "4-6,9".parse().unwrap();
    assert_eq!(parsed.iter().collect::<Vec<_>>(), [4, 5, 6, 9]);
    for bad in [
        "", "0", "5-3", "3,2", "1-3,3", "1-3,4", "a", "1-", "-2", "1,,2", " 1", "+1", "1-2-3",
    ] {
        assert!(bad.parse::<Ranges>().is_err(), "'{bad}' must not parse");
    }
}

#[test]
fn a_print_serializes_compactly_and_whole_hashes_still_read() {
    let json = serde_json::to_string(&recorded()).unwrap();
    assert!(json.starts_with(r#"{"lines":"2,4-5","ranges":""#), "{json}");
    let Print::Lines(LinePrint {
        hash: LineHash::PerRange(hashes),
        ..
    }) = recorded()
    else {
        panic!("a line print");
    };
    assert_eq!(hashes.len(), 2, "one hash per run of executed lines");
    let back: Print = serde_json::from_str(&json).unwrap();
    assert_eq!(back, recorded());
    let whole: Print = serde_json::from_str(r#""blake3:00""#).unwrap();
    assert_eq!(whole, Print::Whole("blake3:00".into()));
    for bad in [
        r#"{"lines":"3,2","hash":"blake3:00"}"#,
        r#"{"lines":"2,4-5","ranges":"0123456789abcdef"}"#,
        r#"{"lines":"2","ranges":"0123"}"#,
        r#"{"lines":"2","ranges":"0123456789abcdef","hash":"blake3:00"}"#,
        r#"{"lines":"2"}"#,
    ] {
        assert!(serde_json::from_str::<Print>(bad).is_err(), "{bad} must not parse");
    }
}

/// The print a receipt written before per-range hashes holds: one joined hash over every executed line.
fn legacy() -> Print {
    let lines: Ranges = "2,4-5".parse().unwrap();
    let hash = content(FILE).line_hash(&lines).unwrap();
    let json = format!(r#"{{"lines":"2,4-5","hash":"{hash}"}}"#);
    serde_json::from_str(&json).unwrap()
}

#[test]
fn a_receipt_from_before_per_range_hashes_reads_by_position() {
    assert!(matches!(
        &legacy(),
        Print::Lines(LinePrint {
            hash: LineHash::Joined(_),
            ..
        })
    ));
    assert!(matches(&legacy(), Some(&content(FILE))));
    assert!(matches(
        &legacy(),
        Some(&content(&FILE.replace("class Deposit", "sealed class Deposit")))
    ));
    assert!(!matches(&legacy(), Some(&content(&FILE.replace("a + b", "a - b")))));
    assert!(
        !matches(&legacy(), Some(&content(&format!("using System;\n{FILE}")))),
        "a shift stays stale for the old format, as it was"
    );
    let json = serde_json::to_string(&legacy()).unwrap();
    assert!(
        json.contains(r#""hash":"blake3:"#),
        "an old print is written back as it was read: {json}"
    );
}

#[test]
fn an_edit_in_an_executed_line_is_stale() {
    let edited = FILE.replace("a + b", "a - b");
    assert!(!matches(&recorded(), Some(&content(&edited))));
    let reindented = FILE.replace("  return total;", "    return total;");
    assert!(
        !matches(&recorded(), Some(&content(&reindented))),
        "whitespace inside a line counts"
    );
}

#[test]
fn an_edit_in_a_line_never_executed_is_current() {
    assert!(matches(&recorded(), Some(&content(FILE))));
    let edited = FILE
        .replace("class Deposit", "sealed class Deposit")
        .replace("{\n", "{ // opens\n");
    assert!(matches(&recorded(), Some(&content(&edited))));
    let appended = format!("{FILE}// a trailing note\n");
    assert!(matches(&recorded(), Some(&content(&appended))));
}

#[test]
fn line_endings_do_not_count() {
    let crlf = FILE.replace('\n', "\r\n");
    assert!(matches(&recorded(), Some(&content(&crlf))));
    let no_final_newline = FILE.trim_end_matches('\n');
    assert!(matches(&recorded(), Some(&content(no_final_newline))));
}

#[test]
fn lines_inserted_or_deleted_around_executed_runs_only_move_them() {
    let inserted = format!("using System;\nusing System.Linq;\n{FILE}");
    assert!(matches(&recorded(), Some(&content(&inserted))), "two lines above");
    let deleted = FILE.replacen("class Deposit\n", "", 1);
    assert!(matches(&recorded(), Some(&content(&deleted))), "a line above deleted");
    let between = FILE.replacen("{\n", "{\n  // why the total\n\n", 1);
    assert!(matches(&recorded(), Some(&content(&between))), "lines between two runs");
    let both = format!("// header\n{}", between.replacen("}\n", "}\n\nclass Other {}\n", 1));
    assert!(matches(&recorded(), Some(&content(&both))));
}

#[test]
fn a_moved_file_is_still_stale_when_an_executed_run_changed() {
    let inserted = format!("using System;\n{}", FILE.replace("a + b", "a - b"));
    assert!(!matches(&recorded(), Some(&content(&inserted))));
    let inside = FILE.replace("  var total = a + b;\n", "  var total = a + b;\n  log(total);\n");
    assert!(
        !matches(&recorded(), Some(&content(&inside))),
        "a line inserted inside an executed run changes that run"
    );
}

#[test]
fn executed_runs_out_of_their_order_are_stale() {
    // Run 1 is `  Map();` (line 2), run 2 the body (lines 4-5). Moving `Map();` below the body keeps every run's
    // text but not their order.
    let reordered = "class Deposit\n{\n  var total = a + b;\n  return total;\n  Map();\n}\n";
    assert!(!matches(&recorded(), Some(&content(reordered))));
    let dropped = "class Deposit\n{\n  var total = a + b;\n  return total;\n}\n";
    assert!(!matches(&recorded(), Some(&content(dropped))), "a run that is gone");
}

#[test]
fn checking_a_long_shifted_file_stays_fast() {
    let body: String = (0..2000).map(|line| format!("  statement_{line}();\n")).collect();
    let file = content(&body);
    let executed: BTreeSet<u32> = (1..=2000).filter(|line| line % 3 != 0).collect();
    let print = print(Some(&file), Some(&executed));
    let shifted = content(&format!("// a\n// b\n{body}"));
    let started = std::time::Instant::now();
    assert!(matches(&print, Some(&shifted)));
    let edited = content(&format!("// a\n{}", body.replace("statement_1999();", "changed();")));
    assert!(!matches(&print, Some(&edited)));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_file_shrunk_below_the_last_executed_line_or_deleted_is_stale() {
    assert!(!matches(
        &recorded(),
        Some(&content("class Deposit\n  Map();\n{\n  var total = a + b;\n"))
    ));
    assert!(!matches(&recorded(), None));
}

#[test]
fn whole_prints_compare_the_whole_file_and_absence() {
    let whole = print(Some(&content(FILE)), None);
    assert!(matches!(whole, Print::Whole(_)));
    assert!(matches(&whole, Some(&content(FILE))));
    assert!(
        !matches(&whole, Some(&content(&format!("{FILE}\n")))),
        "any edit makes a whole print stale"
    );
    let absent = print(None, Some(&set(&[1])));
    assert_eq!(absent, Print::Whole(ABSENT.into()));
    assert!(matches(&absent, None));
    assert!(!matches(&absent, Some(&content(FILE))));
}

#[test]
fn coverage_past_the_end_of_the_file_pins_the_whole_file() {
    let print = print(Some(&content(FILE)), Some(&set(&[2, 40])));
    assert_eq!(print, Print::Whole(content(FILE).hash));
    assert!(matches!(
        super::print(Some(&content(FILE)), Some(&set(&[]))),
        Print::Whole(_)
    ));
}

#[test]
fn the_snapshot_reads_each_file_once_and_knows_what_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.cs"), FILE).unwrap();
    let paths: BTreeSet<String> = ["a.cs".to_string(), "gone.cs".to_string()].into();
    let snapshot = Snapshot::read(dir.path(), &paths);
    assert!(snapshot.get("a.cs").unwrap().is_some());
    assert!(snapshot.get("gone.cs").unwrap().is_none());
    assert!(snapshot.get("other.cs").is_none());

    let executed: BTreeMap<String, BTreeSet<u32>> = [("a.cs".to_string(), set(&[2, 4, 5]))].into();
    let prints = print_all(dir.path(), &paths, &executed);
    assert_eq!(prints["a.cs"], recorded());
    assert_eq!(prints["gone.cs"], Print::Whole(ABSENT.into()));
    assert_eq!(by_lines(&prints), 1);
}
