use super::*;
use crate::proof::lines::Print;

fn raw(file: &str, hash: &str) -> Report {
    Report::Raw(RawReport {
        file: file.into(),
        hash: hash.into(),
    })
}

fn sample() -> Receipt {
    let avp = ["idempotency-key-honored".to_string()];
    Receipt {
        spec: "0001-a".into(),
        runner: "api".into(),
        red: Red {
            commit: "abc".into(),
            patch: None,
            cases: [
                (
                    FmId(2),
                    Entry::new(RedCase::NonDiscriminating, &[], None).with_cases(vec!["FM-2: b".into()], None),
                ),
                (
                    FmId(1),
                    Entry::new(RedCase::Fail, &[], None)
                        .with_cases(vec!["FM-1: a".into()], Some("expected 422, got 200".into())),
                ),
                (FmId(3), Entry::new(RedCase::Fail, &avp, None)),
            ]
            .into(),
            report: raw("evidence/raw/red.xml", "blake3:0a"),
            output: None,
        },
        green: Green {
            commit: "def".into(),
            dirty: false,
            cases: [
                (
                    FmId(1),
                    Entry::new(GreenCase::Pass, &[], Some("ignored for untagged".into()))
                        .with_cases(vec!["FM-1: a".into()], None),
                ),
                (FmId(2), Entry::new(GreenCase::Pass, &[], None)),
                (
                    FmId(3),
                    Entry::new(GreenCase::Pass, &avp, Some("evidence/avp-FM-3.json".into())),
                ),
            ]
            .into(),
            report: raw("evidence/raw/green.xml", "blake3:0b"),
        },
        footprint: [
            ("src/A.cs".into(), Print::Whole("blake3:00".into())),
            (
                "src/B.cs".into(),
                Print::Lines(lines::LinePrint {
                    lines: "3-5,9".parse().unwrap(),
                    hash: lines::LineHash::PerRange(vec!["0123456789abcdef".into(), "fedcba9876543210".into()]),
                }),
            ),
        ]
        .into(),
        footprint_source: Source::Coverage,
        footprint_changed: vec!["src/A.cs".into()],
        inputs: Hashes::new(),
        evidence: Some([("evidence/avp-FM-3.json".into(), "blake3:01".into())].into()),
        ctx_revised: Vec::new(),
        verified_with: BTreeMap::new(),
    }
}

#[test]
fn serializes_in_a_stable_compact_shape() {
    let json = serde_json::to_string(&sample()).unwrap();
    assert_eq!(
        json,
        concat!(
            r#"{"spec":"0001-a","runner":"api","#,
            r#""red":{"commit":"abc","cases":{"FM-1":{"result":"fail","cases":["FM-1: a"],"message":"expected 422, got 200"},"#,
            r#""FM-2":{"result":"non-discriminating","cases":["FM-2: b"]},"FM-3":{"result":"fail","avp":["idempotency-key-honored"]}},"#,
            r#""report":{"file":"evidence/raw/red.xml","hash":"blake3:0a"}},"#,
            r#""green":{"commit":"def","dirty":false,"cases":{"FM-1":{"result":"pass","cases":["FM-1: a"]},"FM-2":{"result":"pass"},"#,
            r#""FM-3":{"result":"pass","avp":["idempotency-key-honored"],"verdict":"evidence/avp-FM-3.json"}},"#,
            r#""report":{"file":"evidence/raw/green.xml","hash":"blake3:0b"}},"#,
            r#""footprint":{"src/A.cs":"blake3:00","src/B.cs":{"lines":"3-5,9","ranges":"0123456789abcdef,fedcba9876543210"}},"#,
            r#""footprint_source":"coverage","footprint_changed":["src/A.cs"],"inputs":{},"evidence":{"evidence/avp-FM-3.json":"blake3:01"}}"#
        )
    );
}

#[test]
fn round_trips_through_json_unchanged() {
    let text = sample().to_text().unwrap();
    let back: Receipt = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.to_text().unwrap(),
        text,
        "reading and writing a receipt changes no byte"
    );
    assert_eq!(back.red.cases[&FmId(2)].result(), RedCase::NonDiscriminating);
    assert_eq!(
        back.red.cases[&FmId(1)].message.as_deref(),
        Some("expected 422, got 200")
    );
    assert_eq!(back.green.cases[&FmId(3)].avp, ["idempotency-key-honored"]);
    assert!(!back.has_committed_reports());
}

#[test]
fn reads_receipts_written_before_compact_evidence() {
    let old = r#"{"spec":"0001-a","runner":"api",
        "red":{"commit":"a","cases":{"FM-1":"fail","FM-2":{"result":"fail","avp":["k"],"verdict":"evidence/red.avp-FM-2.json"}},"report":"evidence/red.trx"},
        "green":{"commit":"b","dirty":false,"cases":{"FM-1":"pass","FM-2":{"result":"pass","avp":["k"],"verdict":"evidence/avp-FM-2.json"}},"report":"evidence/green.trx"},
        "footprint":{},"inputs":{},"evidence":{"evidence/red.trx":"blake3:01"}}"#;
    let receipt: Receipt = serde_json::from_str(old).unwrap();
    assert!(receipt.has_committed_reports());
    assert_eq!(receipt.red.report, Report::Committed("evidence/red.trx".into()));
    assert_eq!(receipt.red.cases[&FmId(1)].result(), RedCase::Fail);
    assert!(receipt.red.cases[&FmId(1)].cases.is_empty());
    assert_eq!(
        receipt.green.cases[&FmId(2)].verdict.as_deref(),
        Some("evidence/avp-FM-2.json")
    );
    assert!(receipt.red.output.is_none());
}

#[test]
fn reads_receipts_written_before_evidence_hashes() {
    let old = r#"{"spec":"0001-a","runner":"api","red":{"commit":"a","cases":{"FM-1":"fail"},"report":"r"},"green":{"commit":"b","dirty":false,"cases":{"FM-1":"pass"},"report":"g"},"footprint":{},"inputs":{}}"#;
    let receipt: Receipt = serde_json::from_str(old).unwrap();
    assert!(receipt.evidence.is_none());
    assert_eq!(receipt.footprint_source, Source::Diff);
    assert!(receipt.footprint_changed.is_empty());
    assert!(receipt.ctx_revised.is_empty());
    assert!(receipt.verified_with.is_empty());
}
