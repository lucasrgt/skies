//! The accessibility floor for Flutter (SKYFL037–040): the static half of what a screen reader needs, on by default
//! in the same spirit as the .NET doctor's CA* security floor and the jsx-a11y floor in `@skiesjs/eslint-plugin`.
//!
//! These rules are Flutter-specific and have no SKYFE twin: on the web, jsx-a11y reads the DOM instead. Each one
//! asks a question the parse can answer with confidence and stays quiet when it cannot (a widget handed in as a
//! variable, a decoration built by a helper, a custom widget whose insides live in another file). A label found
//! on the widget, inside it (`Semantics(label:)`, `semanticLabel:`), or on a `Semantics(label:)` / `Tooltip`
//! around it satisfies every rule. Generated code (`*.g.dart`, `*.freezed.dart`, `lib/l10n/`, a generated
//! `packages/<x>_api/` client) and tests are out of scope.

use super::facts::{Call, Facts};
use super::{Source, code, generated};
use crate::doctor::{Finding, Severity};

/// Named arguments that give a widget (or something inside it) an accessible name.
const LABELS: [&str; 4] = ["semanticLabel", "semanticsLabel", "tooltip", "semanticsTooltip"];

/// Layout and paint widgets that never contribute a spoken label, whatever their arguments. A tap target whose
/// `child` tree is only these (plus icons and images) has nothing for a screen reader to announce.
const SILENT: [&str; 22] = [
    "Align",
    "AspectRatio",
    "Center",
    "CircleAvatar",
    "ClipOval",
    "ClipRRect",
    "ColoredBox",
    "Container",
    "DecoratedBox",
    "Ink",
    "FittedBox",
    "Icon",
    "Image",
    "Opacity",
    "Padding",
    "Placeholder",
    "SizedBox",
    "SvgPicture",
    "Transform",
    "ConstrainedBox",
    "LimitedBox",
    "AnimatedContainer",
];

/// Framework widgets that place a child where it renders. An image inside one of these (or returned by `build`)
/// is on screen as written here; an image handed to anything else (a custom widget's slot, a `map` into a list,
/// a variable, a helper) may be labelled or excluded by its receiver in another file, so it is left alone.
const PLACING: [&str; 46] = [
    "Align",
    "AnimatedContainer",
    "AnimatedOpacity",
    "AnimatedSwitcher",
    "AppBar",
    "AspectRatio",
    "Card",
    "Center",
    "CircleAvatar",
    "ClipOval",
    "ClipPath",
    "ClipRRect",
    "ClipRect",
    "Column",
    "ConstrainedBox",
    "Container",
    "DecoratedBox",
    "Expanded",
    "FittedBox",
    "Flexible",
    "FractionallySizedBox",
    "GestureDetector",
    "GridView",
    "Hero",
    "ImageFiltered",
    "Ink",
    "InkWell",
    "LimitedBox",
    "ListTile",
    "ListView",
    "Material",
    "Opacity",
    "Padding",
    "Positioned",
    "Row",
    "SafeArea",
    "Scaffold",
    "Semantics",
    "SingleChildScrollView",
    "SizedBox",
    "Stack",
    "Transform",
    "Visibility",
    "Wrap",
    "ColoredBox",
    "MergeSemantics",
];

/// Runs the accessibility rules over one production file.
pub fn file(source: &Source) -> Vec<Finding> {
    let path = source.path.to_string_lossy().replace('\\', "/");
    if !source.role.lib || source.role.test || generated(&path) {
        return Vec::new();
    }
    let facts = &source.facts;
    let mut findings = Vec::new();
    let mut add = |rule: &str, severity: Severity, call: &Call, message: &str| {
        findings.push(Finding::new(
            code(rule),
            severity,
            source.path.clone(),
            Some(call.line),
            message.to_string(),
        ));
    };

    for call in facts.calls.iter().filter(|c| is_icon_button(c)) {
        if call.named("tooltip").is_none() && !labelled_inside(facts, call) && !labelled_around(facts, call) {
            add(
                "icon-button-label",
                Severity::Error,
                call,
                "IconButton has no tooltip: a screen reader announces an unlabeled button (tooltip is its label)",
            );
        }
    }
    for call in facts.calls.iter().filter(|c| image_kind(c).is_some()) {
        let label = image_kind(call).unwrap_or_default();
        let excluded = call.named("excludeFromSemantics").is_some_and(|arg| !arg.is_false);
        if call.named(label).is_none()
            && !excluded
            && placed(facts, call)
            && !labelled_around(facts, call)
            && !excluded_around(facts, call)
        {
            add(
                "image-semantics",
                Severity::Error,
                call,
                &format!("image has no {label} and is not excluded from semantics (excludeFromSemantics: true)"),
            );
        }
    }
    for call in facts.calls.iter().filter(|c| is_tap_target(c)) {
        if silent_child(facts, call) && !labelled_around(facts, call) {
            add(
                "tap-target-label",
                Severity::Warning,
                call,
                "tap target shows only icons or images and carries no label: wrap it in Semantics(label:) or a \
                 Tooltip, or give the icon a semanticLabel",
            );
        }
    }
    for call in facts.calls.iter().filter(|c| is_text_field(c)) {
        if unlabelled_decoration(facts, call) && !labelled_around(facts, call) {
            add(
                "text-field-label",
                Severity::Warning,
                call,
                "text field has no labelText, label, or hintText in its decoration: a screen reader announces an \
                 unnamed edit box",
            );
        }
    }
    findings
}

/// Whether `call` builds `widget` (or one of its `named` constructors). The grammar reads an arrow body such as
/// `(photo) => Image.memory(...)` as `.memory` on the receiver `(photo) => Image`, so the receiver is the text
/// after the last arrow.
fn constructor(call: &Call, widget: &str, named: &[&str]) -> bool {
    match receiver(call) {
        None => call.name == widget,
        Some(receiver) => receiver == widget && named.contains(&call.name.as_str()),
    }
}

fn receiver(call: &Call) -> Option<&str> {
    call.receiver
        .as_deref()
        .map(|r| r.rsplit("=>").next().unwrap_or(r).trim())
}

fn is_icon_button(call: &Call) -> bool {
    constructor(call, "IconButton", &["filled", "filledTonal", "outlined"])
}

/// The label argument an image widget takes, when `call` builds one: Flutter's `Image` says `semanticLabel`,
/// flutter_svg's `SvgPicture` says `semanticsLabel`.
fn image_kind(call: &Call) -> Option<&'static str> {
    if constructor(call, "Image", &["asset", "network", "file", "memory"]) {
        Some("semanticLabel")
    } else if constructor(call, "SvgPicture", &["asset", "network", "file", "memory", "string"]) {
        Some("semanticsLabel")
    } else {
        None
    }
}

fn is_tap_target(call: &Call) -> bool {
    (constructor(call, "GestureDetector", &[]) || constructor(call, "InkWell", &[])) && call.named("onTap").is_some()
}

fn is_text_field(call: &Call) -> bool {
    constructor(call, "TextField", &[]) || constructor(call, "TextFormField", &[])
}

fn has_label(call: &Call) -> bool {
    call.args
        .iter()
        .any(|arg| arg.label.as_deref().is_some_and(|l| LABELS.contains(&l)))
}

/// A label inside the widget: `Semantics(label:)`, an icon or image with its own label, or a `Tooltip`.
fn labelled_inside(facts: &Facts, call: &Call) -> bool {
    facts.within(call).any(|inner| {
        has_label(inner) || inner.name == "Tooltip" || (inner.name == "Semantics" && inner.named("label").is_some())
    })
}

/// A `Semantics(label:)`, a `Tooltip`, or a control carrying its own label (`IconButton(tooltip:)`) around the
/// widget names it for assistive technology.
fn labelled_around(facts: &Facts, call: &Call) -> bool {
    facts.around(call).any(|outer| {
        has_label(outer)
            || (receiver(outer).is_none()
                && (outer.name == "Tooltip" || (outer.name == "Semantics" && outer.named("label").is_some())))
    })
}

/// The widget renders where it is written: returned by `build`, or the innermost call around it is a framework
/// widget that places it.
fn placed(facts: &Facts, call: &Call) -> bool {
    if call.built {
        return true;
    }
    let innermost = facts.around(call).min_by_key(|outer| outer.span.1 - outer.span.0);
    innermost.is_some_and(|outer| PLACING.contains(&receiver(outer).unwrap_or(&outer.name)))
}

/// An `ExcludeSemantics` (or `Semantics(excludeSemantics: true)`) around the widget: a decorative subtree.
fn excluded_around(facts: &Facts, call: &Call) -> bool {
    facts.around(call).any(|outer| {
        outer.name == "ExcludeSemantics"
            || (outer.name == "Semantics"
                && (outer.named("excludeSemantics").is_some_and(|arg| arg.is_true) || outer.named("label").is_some()))
    })
}

/// The tap target's `child` tree is fully visible here and made only of silent widgets, with no label anywhere in
/// it. A child passed in as a variable, a helper call, a custom widget, or a `children:` list is unknown, and an
/// unknown child is never flagged.
fn silent_child(facts: &Facts, tap: &Call) -> bool {
    if tap.named("child").is_none() || labelled_inside(facts, tap) {
        return false;
    }
    let inner: Vec<&Call> = facts.within(tap).collect();
    let slots = std::iter::once(tap).chain(inner.iter().copied());
    for call in slots {
        if call.named("children").is_some() {
            return false;
        }
        let Some(child) = call.named("child") else {
            continue;
        };
        let Some(widget) = inner.iter().find(|c| c.span.0 == child.start) else {
            return false;
        };
        let name = receiver(widget).unwrap_or(&widget.name);
        if !SILENT.contains(&name) {
            return false;
        }
    }
    true
}

/// No decoration at all, or an `InputDecoration(...)` written here with no `labelText`, `label`, or `hintText`.
/// A decoration built elsewhere (a variable, a helper, `copyWith`) may carry the label, so it is never flagged.
fn unlabelled_decoration(facts: &Facts, field: &Call) -> bool {
    let Some(decoration) = field.named("decoration") else {
        return true;
    };
    let built = facts.within(field).find(|c| {
        c.span.0 == decoration.start && (c.name == "InputDecoration" || receiver(c) == Some("InputDecoration"))
    });
    built.is_some_and(|d| ["labelText", "label", "hintText"].iter().all(|l| d.named(l).is_none()))
}
