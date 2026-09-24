//! The accessibility floor (SKYFL037–040): each rule fires on the unlabeled shape, stays quiet on every way the
//! widget can be named, and never guesses about a child or decoration it cannot see.

use super::diagnose;
use crate::doctor::{Finding, Severity};

const VIEW: &str = "lib/features/x/x_view.dart";

/// Diagnoses one file. A path under `packages/<x>/` makes that folder the package, as `--package` would.
fn findings(path: &str, body: &str) -> Vec<Finding> {
    findings_in(path, &format!("Widget build(BuildContext context) => {body};"))
}

fn findings_in(path: &str, source: &str) -> Vec<Finding> {
    let dir = tempfile::tempdir().unwrap();
    let package = match path.strip_prefix("packages/").and_then(|rest| rest.split_once('/')) {
        Some((name, _)) => dir.path().join("packages").join(name),
        None => dir.path().to_path_buf(),
    };
    let file = dir.path().join(path);
    std::fs::create_dir_all(package.join("lib")).unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, format!("{source}\n")).unwrap();
    diagnose(&package)
        .unwrap()
        .into_iter()
        .filter(|f| f.code.as_str() >= "SKYFL037")
        .collect()
}

fn codes(body: &str) -> Vec<String> {
    findings("lib/ui/widget.dart", body)
        .into_iter()
        .map(|f| f.code)
        .collect()
}

fn fires(code: &str, bodies: &[&str]) {
    for body in bodies {
        assert_eq!(codes(body), [code], "{code} should fire on: {body}");
    }
}

fn clean(bodies: &[&str]) {
    for body in bodies {
        assert_eq!(codes(body), Vec::<String>::new(), "should be clean: {body}");
    }
}

#[test]
fn skyfl037_an_icon_button_needs_a_tooltip() {
    fires(
        "SKYFL037",
        &[
            "IconButton(icon: const Icon(Icons.close), onPressed: close)",
            "IconButton.filled(icon: Icon(Icons.add), onPressed: add)",
            // A Semantics around it that names nothing is not a label.
            "Semantics(button: true, child: IconButton(icon: Icon(Icons.add), onPressed: add))",
        ],
    );
    clean(&[
        "IconButton(tooltip: l10n.close, icon: const Icon(Icons.close), onPressed: close)",
        // The design-system shape: the icon is labelled inside the button.
        "IconButton(icon: Semantics(label: tooltip, excludeSemantics: true, child: icon), onPressed: onPressed)",
        "IconButton(icon: Icon(Icons.close, semanticLabel: l10n.close), onPressed: close)",
        "Tooltip(message: l10n.close, child: IconButton(icon: Icon(Icons.close), onPressed: close))",
        // Lookalikes: another receiver's constructor, and the name inside a string or a comment.
        "other.IconButton(icon: x)",
        "Text('IconButton(icon: x)') /* IconButton(icon: x) */",
    ]);
}

#[test]
fn skyfl038_an_image_is_labelled_or_excluded() {
    fires(
        "SKYFL038",
        &[
            "Image(image: provider(url), fit: BoxFit.cover)",
            "const Image(image: AssetImage('a.png'))",
            "Image.asset('assets/logo.png', width: 40)",
            "Image.network(url)",
            "Image.memory(bytes, gaplessPlayback: true)",
            "Image.file(file)",
            "SvgPicture.asset('assets/logo.svg')",
            "SvgPicture.string(svg, width: 20)",
            "Image.asset('a.png', excludeFromSemantics: false)",
            // Placed by a framework widget, however deep.
            "SizedBox(width: 20, child: Image.network(url))",
            "Row(children: [Text(name), ClipRRect(borderRadius: r, child: Image.memory(bytes))])",
            "cond ? Image.asset('a.png') : const SizedBox.shrink()",
        ],
    );
    clean(&[
        "Image.asset('assets/logo.png', semanticLabel: l10n.logo)",
        "Image(image: photo, excludeFromSemantics: true)",
        "Image.memory(bytes, excludeFromSemantics: decorative)",
        "SvgPicture.asset('assets/logo.svg', semanticsLabel: l10n.logo)",
        "SvgPicture.string(svg, excludeFromSemantics: true)",
        "ExcludeSemantics(child: Image.network(url))",
        "Semantics(label: l10n.photo, child: Image.memory(bytes))",
        "Semantics(excludeSemantics: true, child: Image.memory(bytes))",
        "IconButton(tooltip: l10n.logo, icon: Image.asset('a.png'), onPressed: f)",
        // Not the widget: an image provider and a library's own Image type.
        "CircleAvatar(backgroundImage: NetworkImage(url))",
        "img.Image(width: 2, height: 2)",
        // Handed to a receiver that may label or exclude it in another file: a custom widget's slot, a list built
        // by `map` (an arrow body the grammar reads as `.memory` on `(photo) => Image`).
        "ServiceGallery(cover: Image.memory(bytes))",
        "ServiceGallery(extras: photos.map((photo) => Image.memory(photo.bytes)).toList())",
    ]);
    let handed_off = "class Panel {\n  Widget build(BuildContext context) {\n    final cover = Image.memory(bytes);\n    return Gallery(cover: cover);\n  }\n  Widget photo(Photo p) => Image.memory(p.bytes);\n}";
    assert!(findings_in("lib/ui/panel.dart", handed_off).is_empty());
    let returned = "class Photo {\n  Widget build(BuildContext context) {\n    if (url.isEmpty) return fallback;\n    return Image(image: provider(url));\n  }\n}";
    let found = findings_in("lib/ui/photo.dart", returned);
    assert_eq!(
        found.iter().map(|f| (f.code.as_str(), f.line)).collect::<Vec<_>>(),
        [("SKYFL038", Some(4))]
    );
}

#[test]
fn skyfl039_a_tap_target_with_only_icons_needs_a_label() {
    fires(
        "SKYFL039",
        &[
            "GestureDetector(onTap: close, child: const Icon(Icons.close))",
            "InkWell(onTap: open, child: Padding(padding: EdgeInsets.all(8), child: Icon(Icons.menu)))",
            "InkWell(onTap: open, child: SizedBox.square(dimension: 40, child: Center(child: Icon(Icons.menu))))",
        ],
    );
    clean(&[
        "InkWell(onTap: open, child: Padding(padding: p, child: Text(l10n.open)))",
        "InkWell(onTap: open, child: Icon(Icons.menu, semanticLabel: l10n.menu))",
        "Semantics(label: l10n.menu, button: true, child: InkWell(onTap: open, child: Icon(Icons.menu)))",
        "Tooltip(message: l10n.menu, child: GestureDetector(onTap: open, child: Icon(Icons.menu)))",
        // Children it cannot see are never guessed at: a variable, a custom widget, a helper, a list.
        "InkWell(onTap: open, child: child)",
        "InkWell(onTap: open, child: PropertyCard(property: p))",
        "InkWell(onTap: open, child: _tile(context))",
        "InkWell(onTap: open, child: Row(children: [Icon(Icons.menu), label]))",
        // No tap handler, no tap target.
        "GestureDetector(onPanUpdate: drag, child: Icon(Icons.drag_handle))",
    ]);
}

#[test]
fn skyfl040_a_text_field_is_named_by_its_decoration() {
    fires(
        "SKYFL040",
        &[
            "TextField(controller: c)",
            "TextFormField(controller: c, validator: v)",
            "TextField(decoration: InputDecoration(prefixIcon: Icon(Icons.search)))",
            "TextField(decoration: const InputDecoration(border: InputBorder.none))",
        ],
    );
    clean(&[
        "TextField(decoration: InputDecoration(labelText: l10n.email))",
        "TextField(decoration: InputDecoration(label: Text(l10n.email)))",
        "TextFormField(decoration: InputDecoration(hintText: l10n.search), validator: v)",
        "TextField(decoration: InputDecoration.collapsed(hintText: l10n.search))",
        // A decoration built elsewhere may carry the label.
        "TextField(decoration: decoration)",
        "TextField(decoration: appInputDecoration(context, label: l10n.email))",
        "TextField(decoration: base.copyWith(prefixIcon: icon))",
        "Semantics(label: l10n.email, child: TextField(controller: c))",
    ]);
}

#[test]
fn severities_are_error_for_the_decisive_shapes_and_warning_for_the_heuristics() {
    let found = findings(
        VIEW,
        "Column(children: [IconButton(icon: i, onPressed: f), Image.network(u), InkWell(onTap: f, child: Icon(i)), TextField()])",
    );
    let severity = |code: &str| found.iter().find(|f| f.code == code).map(|f| f.severity);
    assert_eq!(severity("SKYFL037"), Some(Severity::Error));
    assert_eq!(severity("SKYFL038"), Some(Severity::Error));
    assert_eq!(severity("SKYFL039"), Some(Severity::Warning));
    assert_eq!(severity("SKYFL040"), Some(Severity::Warning));
    assert!(found.iter().all(|f| f.line == Some(1)));
}

#[test]
fn generated_code_and_tests_are_out_of_scope() {
    let unlabeled = "IconButton(icon: i, onPressed: f)";
    for path in [
        "lib/models/x.g.dart",
        "lib/models/x.freezed.dart",
        "lib/l10n/app_localizations.dart",
        "packages/sample_api/lib/api.dart",
        "test/x_test.dart",
    ] {
        assert!(findings(path, unlabeled).is_empty(), "{path} should be skipped");
    }
    assert_eq!(findings("lib/features/x/x_view.dart", unlabeled).len(), 1);
    assert_eq!(findings("packages/app_ui/lib/button.dart", unlabeled).len(), 1);
}
