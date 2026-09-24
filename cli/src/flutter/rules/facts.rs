//! The facts the SKYFL rules read, extracted from a real Dart parse (tree-sitter-dart).
//!
//! The 4.x doctor matched regexes against raw source, so a `Text('...')` inside a comment, a `TODO` inside a
//! string, or a call split across lines decided findings. Here every fact comes from a syntax node: imports are
//! import URIs, identifiers are code identifiers, calls know their receiver, arguments, and the calls that
//! enclose them. Rules then ask questions of these facts instead of re-parsing text.

use std::collections::BTreeSet;

/// A value with the 1-based line it was found on.
#[derive(Debug, Clone)]
pub struct Located<T> {
    pub value: T,
    pub line: usize,
}

/// One argument of a call or object creation.
#[derive(Debug, Clone, Default)]
pub struct Arg {
    /// The label of a named argument (`baseUrl` in `baseUrl: x`), `None` for positional ones.
    pub label: Option<String>,
    /// The literal's content when the value is a plain string literal without interpolation.
    pub string: Option<String>,
    /// The identifier when the value is a bare identifier.
    pub identifier: Option<String>,
    /// The target type when the value is a cast (`route as dynamic` → `dynamic`).
    pub cast: Option<String>,
    /// The value is the boolean literal `true`.
    pub is_true: bool,
}

/// A call, constructor call, or `const`/`new` object creation.
#[derive(Debug, Clone)]
pub struct Call {
    /// The invoked name: the function, the method (`pop` in `Navigator.of(context).pop()`), or the type.
    pub name: String,
    /// The receiver's source text for a method call (`Navigator.of(context)`).
    pub receiver: Option<String>,
    /// Explicit type arguments at the call (`ResourceBuilder<List<X>>(...)`).
    pub generic: bool,
    pub args: Vec<Arg>,
    /// Names of the calls whose argument lists contain this one, innermost last.
    pub enclosing: Vec<String>,
    /// Inside the condition of an `if`.
    pub in_condition: bool,
    /// The generated-client shape `<x>.api.get<Tag>Api().<operation>(...)`.
    pub is_operation: bool,
    pub line: usize,
}

impl Call {
    pub fn positional(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|arg| arg.label.is_none())
    }

    pub fn named(&self, label: &str) -> Option<&Arg> {
        self.args.iter().find(|arg| arg.label.as_deref() == Some(label))
    }
}

/// A function or method signature: its name and the head of its return type (`Future` in `Future<void>`).
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub returns: String,
    pub line: usize,
}

/// A subscript read such as `state.pathParameters['id']`.
#[derive(Debug, Clone)]
pub struct IndexRead {
    /// The subscripted expression's source text.
    pub object: String,
    /// The index expression's source text, quotes included.
    pub index: String,
    pub line: usize,
}

/// A callback bound to a name (`onSuccess: () {...}`, `final onSuccess = ...`, `x.onSuccess = ...`).
#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    /// Identifiers used inside the bound value.
    pub identifiers: BTreeSet<String>,
    pub line: usize,
}

/// Everything the rules know about one Dart file.
#[derive(Debug, Default)]
pub struct Facts {
    pub imports: Vec<Located<String>>,
    pub identifiers: Vec<Located<String>>,
    /// Type names written with type arguments (`AsyncState<int>`, `ResourceBuilder<List<X>>(...)`).
    pub generics: BTreeSet<String>,
    pub calls: Vec<Call>,
    pub comments: Vec<Located<String>>,
    pub signatures: Vec<Signature>,
    /// The identifiers used in each `catch` block.
    pub catch_blocks: Vec<BTreeSet<String>>,
    pub index_reads: Vec<IndexRead>,
    pub bindings: Vec<Binding>,
    pub has_else: bool,
}

impl Facts {
    pub fn parse(source: &str) -> Facts {
        super::syntax::extract(source)
    }

    pub fn has_identifier(&self, name: &str) -> bool {
        self.identifiers.iter().any(|id| id.value == name)
    }

    pub fn first_identifier(&self, names: &[&str]) -> Option<&Located<String>> {
        self.identifiers.iter().find(|id| names.contains(&id.value.as_str()))
    }

    pub fn calls_named<'a>(&'a self, names: &'a [&str]) -> impl Iterator<Item = &'a Call> {
        self.calls
            .iter()
            .filter(move |call| names.contains(&call.name.as_str()))
    }
}
