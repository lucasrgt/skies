//! The `skies-sdd` skill ships twice: in the agent plugin and inside every app `skies new` creates. The two copies
//! must never drift, so an agent reads the same delivery process wherever it meets Skies.

#[test]
fn the_app_template_carries_the_plugin_skill_verbatim() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let plugin = std::fs::read_to_string(root.join("skies-plugin/skills/skies-sdd.md")).unwrap();
    let template = std::fs::read_to_string(root.join("cli/templates/app/.claude/skills/skies-sdd/SKILL.md")).unwrap();
    assert_eq!(
        plugin, template,
        "copy skies-plugin/skills/skies-sdd.md into the app template"
    );
}
