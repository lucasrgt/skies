//! The web UI kit ships twice: in the sample's web package, where spec 0008 proves it, and in every package
//! `skies g web-app` creates. The copies must never drift, so what a new app starts from is the kit the sample's
//! receipt proves.

#[test]
fn the_web_app_template_carries_the_sample_kit_verbatim() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let sample = root.join("examples/sample-app/frontend/web/src/ui");
    let template = root.join("cli/templates/react/app/src/ui");
    let mut names: Vec<_> = std::fs::read_dir(&sample)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    let mut theirs: Vec<_> = std::fs::read_dir(&template)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    theirs.sort();
    assert_eq!(names, theirs, "the template's src/ui holds the sample kit's files");
    for name in names {
        assert_eq!(
            std::fs::read_to_string(sample.join(&name)).unwrap(),
            std::fs::read_to_string(template.join(&name)).unwrap(),
            "copy examples/sample-app/frontend/web/src/ui/{} into cli/templates/react/app/src/ui",
            name.to_string_lossy()
        );
    }
}
