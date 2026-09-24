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
    assert!(json.starts_with(r#"{"lines":"2,4-5","hash":"blake3:"#), "{json}");
    let back: Print = serde_json::from_str(&json).unwrap();
    assert_eq!(back, recorded());
    let whole: Print = serde_json::from_str(r#""blake3:00""#).unwrap();
    assert_eq!(whole, Print::Whole("blake3:00".into()));
    assert!(serde_json::from_str::<Print>(r#"{"lines":"3,2","hash":"blake3:00"}"#).is_err());
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
fn a_line_inserted_above_executed_lines_shifts_them_and_is_stale() {
    let inserted = format!("using System;\n{FILE}");
    assert!(!matches(&recorded(), Some(&content(&inserted))));
    let deleted = FILE.replacen("class Deposit\n", "", 1);
    assert!(!matches(&recorded(), Some(&content(&deleted))));
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
