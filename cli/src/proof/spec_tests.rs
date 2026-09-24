use super::*;

const SAMPLE: &str = "---\nid: \"0012\"   # the folder id\nrunner: web\ntouches: [src/Res/**, 'src/Pay.cs']\n---\n# Cancel\n\nCancel mentions FM-9 in prose.\n\n## Failure modes\n\n- FM-1 other guest -> 404\n- FM-2: twice -> one refund\n* FM-3 after check-in -> 409\nSee FM-7 in passing.\n\n## Non-discriminating\n\n- FM-2 the refund ledger already dedupes at the base revision.\n\n## Out of scope\n- FM-8 is not a failure mode here\n";

#[test]
fn reads_frontmatter_and_failure_modes() {
    let doc = parse(SAMPLE).unwrap();
    assert_eq!(doc.id.as_deref(), Some("0012"));
    assert_eq!(doc.runner.as_deref(), Some("web"));
    assert_eq!(doc.touches, ["src/Res/**", "src/Pay.cs"]);
    assert_eq!(doc.failure_modes, [FmId(1), FmId(2), FmId(3)]);
    assert_eq!(doc.justified, [FmId(2)]);
}

#[test]
fn reads_avp_tags_and_keeps_them_out_of_the_text() {
    let doc = parse(
        "## Failure modes\n- FM-1 plain\n- FM-5: a retry credits twice [avp: idempotency-key-honored]\n\
         - FM-6 two tags [AVP: a-b, c] trailing\n",
    )
    .unwrap();
    assert!(doc.modes[&FmId(1)].avp.is_empty());
    assert_eq!(doc.modes[&FmId(1)].text, "plain");
    assert_eq!(doc.modes[&FmId(5)].text, "a retry credits twice");
    assert_eq!(doc.modes[&FmId(5)].avp, ["idempotency-key-honored"]);
    assert_eq!(doc.modes[&FmId(6)].avp, ["a-b", "c"]);
    assert_eq!(doc.modes[&FmId(6)].text, "two tags trailing");
}

#[test]
fn rejects_malformed_avp_tags() {
    for line in ["- FM-1 x [avp: ]", "- FM-1 x [avp: Not Kebab]", "- FM-1 x [avp: a"] {
        let error = parse(&format!("## Failure modes\n{line}\n")).unwrap_err();
        assert!(format!("{error:#}").contains("FM-1"), "{line}: {error:#}");
    }
}

#[test]
fn a_failure_mode_continues_on_indented_lines() {
    let doc = parse(
        "## Failure modes\n\n- FM-1 `ErrorBody.code` does not enumerate the app's error codes (the Wallets and\n  \
         Money registries), so the client cannot type them.\n- FM-2 A retry credits twice\n\t[avp: key-honored]\n\
         - FM-3 one line\n\n  An indented paragraph after a blank line is prose, not FM-3.\n* FM-4 last\n  line\n",
    )
    .unwrap();
    assert_eq!(doc.failure_modes, [FmId(1), FmId(2), FmId(3), FmId(4)]);
    assert_eq!(
        doc.modes[&FmId(1)].text,
        "`ErrorBody.code` does not enumerate the app's error codes (the Wallets and Money registries), so the client \
         cannot type them."
    );
    assert_eq!(doc.modes[&FmId(2)].text, "A retry credits twice");
    assert_eq!(
        doc.modes[&FmId(2)].avp,
        ["key-honored"],
        "a tag on a continuation line counts"
    );
    assert_eq!(doc.modes[&FmId(3)].text, "one line");
    assert_eq!(doc.modes[&FmId(4)].text, "last line");
}

#[test]
fn reads_dashed_touches() {
    let doc = parse("---\nid: 1\nrunner: api\ntouches:\n  - a/**\n  - \"b.cs\"\n---\n").unwrap();
    assert_eq!(doc.touches, ["a/**", "b.cs"]);
}

#[test]
fn rejects_duplicate_failure_modes() {
    let error = parse("## Failure modes\n- FM-1 a\n- FM-1 b\n").unwrap_err();
    assert!(error.to_string().contains("FM-1 is listed twice"));
}

#[test]
fn the_template_parses_back() {
    let doc = parse(&template("0004", "cancel-reservation", "api")).unwrap();
    assert_eq!(doc.id.as_deref(), Some("0004"));
    assert_eq!(doc.runner.as_deref(), Some("api"));
    assert!(doc.touches.is_empty());
    assert_eq!(doc.failure_modes, [FmId(1)]);
    assert!(template("0004", "cancel-reservation", "api").contains("# Cancel reservation"));
}

#[test]
fn numbers_and_finds_specs() {
    let root = tempfile::tempdir().unwrap();
    for name in ["0001-a", "0009-notes", "0010-b"] {
        std::fs::create_dir_all(root.path().join(SPECS_DIR).join(name)).unwrap();
    }
    for name in ["0001-a", "0010-b"] {
        std::fs::write(root.path().join(SPECS_DIR).join(name).join(SPEC_FILE), "").unwrap();
    }
    assert_eq!(next_id(root.path()).unwrap(), "0011");
    let names: Vec<String> = discover(root.path())
        .unwrap()
        .into_iter()
        .map(|spec| spec.name)
        .collect();
    assert_eq!(names, ["0001-a", "0010-b"]);
    assert_eq!(find(root.path(), "10").unwrap().name, "0010-b");
    assert_eq!(find(root.path(), "0001-a").unwrap().id, "0001");
    assert!(find(root.path(), "0009").is_err());
}

#[test]
fn validates_slugs() {
    assert!(validate_slug("cancel-reservation").is_ok());
    assert!(validate_slug("Cancel").is_err());
    assert!(validate_slug("-x").is_err());
    assert!(validate_slug("a b").is_err());
}
