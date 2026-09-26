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
fn the_template_parses_back_with_one_placeholder_mode() {
    let doc = parse(&template("0004", "cancel-reservation", "api")).unwrap();
    assert_eq!(doc.id.as_deref(), Some("0004"));
    assert_eq!(doc.runner.as_deref(), Some("api"));
    assert!(doc.touches.is_empty());
    assert_eq!(doc.failure_modes, [FmId(1)]);
    assert!(doc.modes[&FmId(1)].text.starts_with("<replace with what goes wrong"));
    assert!(template("0004", "cancel-reservation", "api").contains("# Cancel reservation"));
}

#[test]
fn look_alike_failure_modes_are_errors_not_silence() {
    for line in [
        "- FM 3 spaced",
        "- fm_3 underscored",
        "- FM-[rejected-update]: named",
        "FM-3 no bullet",
    ] {
        let error = parse(&format!("## Failure modes\n\n- FM-1 fine\n{line}\n")).unwrap_err();
        assert!(
            format!("{error:#}").contains("is not a failure-mode line"),
            "{line}: {error:#}"
        );
    }
    let justified = parse("## Failure modes\n- FM-1 a\n## Non-discriminating\n- FM 1 spaced\n").unwrap_err();
    assert!(format!("{justified:#}").contains("- FM-<n>"));
    let prose = parse("# X\n\nFM 3 outside the section is prose.\n## Failure modes\n- FM-1 a\n").unwrap();
    assert_eq!(prose.failure_modes, [FmId(1)]);
}

#[test]
fn a_spec_without_failure_modes_says_how_to_write_one() {
    let spec = SpecDir {
        name: "0001-x".into(),
        id: "0001".into(),
        path: PathBuf::from("/nowhere"),
    };
    let message = parse("## Failure modes\n\nNone yet.\n")
        .unwrap()
        .unprovable(&spec)
        .unwrap();
    assert!(message.contains("lists no failure mode"), "{message}");
    assert!(message.contains("`- FM-1 <what goes wrong>`"), "{message}");
    assert!(
        parse("## Failure modes\n- FM-1 a [avp: criterion]\n")
            .unwrap()
            .unprovable(&spec)
            .is_none()
    );

    let placeholder = parse(&template("0001", "x", "api")).unwrap().unprovable(&spec).unwrap();
    assert!(
        placeholder.contains("FM-1 still reads as the template's placeholder"),
        "{placeholder}"
    );
}

#[test]
fn two_spec_folders_with_one_id_are_refused() {
    let root = tempfile::tempdir().unwrap();
    for name in ["0001-auth", "0001-home", "2-b", "0002-c"] {
        let dir = root.path().join(SPECS_DIR).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SPEC_FILE), "").unwrap();
    }
    let error = format!("{:#}", find(root.path(), "0001-home").unwrap_err());
    assert!(
        error.contains(".specs/0001-auth and .specs/0001-home share id 0001"),
        "{error}"
    );
    assert!(error.contains(".specs/0002-c and .specs/2-b"), "{error}");
    assert!(error.contains("next free id (0003"), "{error}");
}

/// Every spec.md this repository ships (the sample's, the generator snapshots') follows the grammar and lists modes.
#[test]
fn every_shipped_spec_follows_the_grammar() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = 0;
    for entry in walkdir::WalkDir::new(&repo).into_iter().filter_entry(|entry| {
        !matches!(
            entry.file_name().to_str(),
            Some("node_modules" | "target" | ".git" | "worktrees")
        )
    }) {
        let entry = entry.unwrap();
        let path = entry.path();
        let in_specs = path.parent().and_then(Path::parent).and_then(Path::file_name) == Some(".specs".as_ref());
        if entry.file_name() != SPEC_FILE || !in_specs {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        let doc = parse(&text).unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        assert!(
            !doc.failure_modes.is_empty(),
            "{} lists no failure mode",
            path.display()
        );
        assert!(
            doc.modes.values().all(|mode| !mode.text.starts_with(PLACEHOLDER)),
            "{} still holds the template's placeholder",
            path.display()
        );
        found += 1;
    }
    assert!(found >= 10, "found only {found} specs");
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
