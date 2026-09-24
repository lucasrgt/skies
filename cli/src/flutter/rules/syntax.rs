//! The tree-sitter walk that fills [`Facts`]. It is the only code that knows the Dart grammar's node kinds and
//! field names, so a grammar upgrade touches this file and nothing else.

use std::collections::BTreeSet;

use tree_sitter::{Node, Parser};

use super::facts::{Arg, Binding, Call, Facts, IndexRead, Located, Signature};

/// Parses `source` and extracts its facts. A file the grammar cannot fully parse still yields the facts of the
/// parts it could: tree-sitter recovers around errors, which is what a doctor over work-in-progress code needs.
pub fn extract(source: &str) -> Facts {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .expect("the bundled Dart grammar loads");
    let mut facts = Facts::default();
    if let Some(tree) = parser.parse(source, None) {
        let mut walker = Walker {
            src: source.as_bytes(),
            facts: &mut facts,
            enclosing: Vec::new(),
            in_condition: 0,
        };
        walker.visit(tree.root_node());
    }
    facts
}

struct Walker<'a> {
    src: &'a [u8],
    facts: &'a mut Facts,
    enclosing: Vec<String>,
    in_condition: usize,
}

impl Walker<'_> {
    fn text(&self, node: Node) -> String {
        node.utf8_text(self.src).unwrap_or_default().to_string()
    }

    fn located(&self, node: Node, value: String) -> Located<String> {
        Located {
            value,
            line: node.start_position().row + 1,
        }
    }

    fn visit(&mut self, node: Node) {
        match node.kind() {
            "comment" | "documentation_comment" => {
                let comment = self.located(node, self.text(node));
                self.facts.comments.push(comment);
                return;
            }
            "import_or_export" => {
                if let Some(uri) = find(node, "string_literal").and_then(|s| string_content(s, self.src)) {
                    let import = self.located(node, uri);
                    self.facts.imports.push(import);
                }
                return;
            }
            "identifier" | "type_identifier" | "identifier_dollar_escaped" => {
                let id = self.located(node, self.text(node));
                self.facts.identifiers.push(id);
            }
            "type_arguments" => {
                if let Some(previous) = node.prev_named_sibling().filter(|p| p.kind() == "type_identifier") {
                    self.facts.generics.insert(self.text(previous));
                }
            }
            "try_statement" => self.record_catch_blocks(node),
            "if_statement" => return self.visit_if(node),
            "else" => self.facts.has_else = true,
            "index_expression" | "assignable_expression" => self.record_index(node),
            "function_signature" => self.record_signature(node),
            "named_argument"
            | "static_final_declaration"
            | "initialized_variable_definition"
            | "initialized_identifier"
            | "assignment_expression" => self.record_binding(node),
            "call_expression" | "const_object_expression" | "new_expression" => {
                return self.visit_call(node);
            }
            _ => {}
        }
        self.visit_children(node);
    }

    fn visit_children(&mut self, node: Node) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child);
        }
    }

    /// Everything before the consequence is the condition; `in_condition` marks calls made there.
    fn visit_if(&mut self, node: Node) {
        let consequence = node
            .child_by_field_name("consequence")
            .map(|c| c.start_byte())
            .unwrap_or(usize::MAX);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let condition = child.start_byte() < consequence;
            self.in_condition += usize::from(condition);
            if child.kind() == "else" {
                self.facts.has_else = true;
            }
            self.visit(child);
            self.in_condition -= usize::from(condition);
        }
    }

    fn visit_call(&mut self, node: Node) {
        let (name, receiver, generic) = match node.child_by_field_name("function") {
            Some(function) => self.callee(function),
            None => {
                // `const Foo<T>(...)` / `new Foo<T>(...)`: the grammar gives the type and its arguments as two
                // `type` fields.
                let types: Vec<Node> = children_by_field(node, "type");
                let name = types
                    .first()
                    .and_then(|t| find(*t, "type_identifier"))
                    .map(|t| self.text(t));
                (name.unwrap_or_default(), None, types.len() > 1)
            }
        };
        if generic && !name.is_empty() {
            self.facts.generics.insert(name.clone());
        }
        let arguments = node.child_by_field_name("arguments");
        let call = Call {
            args: arguments.map(|a| self.arguments(a)).unwrap_or_default(),
            is_operation: self.is_operation(node),
            enclosing: self.enclosing.clone(),
            in_condition: self.in_condition > 0,
            line: node.start_position().row + 1,
            span: (node.start_byte(), node.end_byte()),
            built: self.returned_by_build(node),
            name: name.clone(),
            receiver,
            generic,
        };
        self.facts.calls.push(call);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let is_arguments = Some(child) == arguments;
            if is_arguments {
                self.enclosing.push(name.clone());
            }
            self.visit(child);
            if is_arguments {
                self.enclosing.pop();
            }
        }
    }

    /// Whether `call` is the value a `build` method returns: the arrow body or a `return` operand, through
    /// parentheses and either branch of a conditional. A lambda in between (a builder callback) is not `build`.
    fn returned_by_build(&self, call: Node) -> bool {
        let mut node = call;
        while let Some(parent) = node.parent() {
            if !matches!(parent.kind(), "conditional_expression" | "parenthesized_expression") {
                break;
            }
            node = parent;
        }
        if !node
            .parent()
            .is_some_and(|p| matches!(p.kind(), "return_statement" | "function_body"))
        {
            return false;
        }
        let mut current = node.parent();
        while let Some(ancestor) = current {
            match ancestor.kind() {
                "function_expression" | "lambda_expression" => return false,
                "method_declaration" | "function_declaration" | "local_function_declaration" => {
                    return find(ancestor, "function_signature")
                        .and_then(|s| s.child_by_field_name("name"))
                        .is_some_and(|name| self.text(name) == "build");
                }
                _ => current = ancestor.parent(),
            }
        }
        false
    }

    /// Resolves a call's `function` node to (name, receiver text, has type arguments).
    fn callee(&self, function: Node) -> (String, Option<String>, bool) {
        match function.kind() {
            "identifier" => (self.text(function), None, false),
            "member_expression" => {
                let name = function
                    .child_by_field_name("property")
                    .map(|p| self.text(p))
                    .unwrap_or_default();
                let receiver = function.child_by_field_name("object").map(|o| self.text(o));
                (name, receiver, false)
            }
            "instantiation_expression" => {
                let inner = function.child_by_field_name("function");
                let (name, receiver, _) = inner.map(|f| self.callee(f)).unwrap_or_default();
                (name, receiver, true)
            }
            // The grammar reads `(c) => Text('x')` as a call of the arrow function with `('x')`, so the invoked
            // name is the arrow's body expression.
            "function_expression" => function
                .child_by_field_name("body")
                .and_then(|body| body.named_child(body.named_child_count().saturating_sub(1) as u32))
                .map(|expression| self.callee(expression))
                .unwrap_or_default(),
            _ => (String::new(), None, false),
        }
    }

    /// `<x>.api.get<Tag>Api().<operation>(...)`: the only way generated operations are reached.
    fn is_operation(&self, call: Node) -> bool {
        let property_of = |node: Node| node.child_by_field_name("property").map(|p| self.text(p));
        let Some(member) = call
            .child_by_field_name("function")
            .filter(|f| f.kind() == "member_expression")
        else {
            return false;
        };
        let Some(inner) = member
            .child_by_field_name("object")
            .filter(|o| o.kind() == "call_expression")
        else {
            return false;
        };
        let empty_args = inner
            .child_by_field_name("arguments")
            .is_some_and(|a| a.named_child_count() == 0);
        let Some(getter) = inner
            .child_by_field_name("function")
            .filter(|f| f.kind() == "member_expression")
        else {
            return false;
        };
        let tag_api = property_of(getter).is_some_and(|name| {
            name.len() > 6
                && name.starts_with("get")
                && name.ends_with("Api")
                && name[3..].starts_with(|c: char| c.is_ascii_uppercase())
        });
        let on_api = getter
            .child_by_field_name("object")
            .filter(|o| o.kind() == "member_expression")
            .and_then(property_of)
            .is_some_and(|name| name == "api");
        empty_args && tag_api && on_api
    }

    fn arguments(&self, node: Node) -> Vec<Arg> {
        let mut out = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            let (label, value) = if child.kind() == "named_argument" {
                let label = find(child, "label")
                    .and_then(|l| find(l, "identifier"))
                    .map(|i| self.text(i));
                (
                    label,
                    child.named_child(child.named_child_count().saturating_sub(1) as u32),
                )
            } else {
                (None, Some(child))
            };
            let Some(value) = value else { continue };
            let mut arg = match value.kind() {
                "string_literal" => Arg {
                    label,
                    string: string_content(value, self.src),
                    ..Arg::default()
                },
                "identifier" => Arg {
                    label,
                    identifier: Some(self.text(value)),
                    ..Arg::default()
                },
                "true" => Arg {
                    label,
                    is_true: true,
                    ..Arg::default()
                },
                "type_cast_expression" => {
                    let target = find(value, "type_cast")
                        .and_then(|c| find(c, "type"))
                        .map(|t| self.text(t));
                    Arg {
                        label,
                        cast: target,
                        ..Arg::default()
                    }
                }
                "false" => Arg {
                    label,
                    is_false: true,
                    ..Arg::default()
                },
                _ => Arg {
                    label,
                    ..Arg::default()
                },
            };
            arg.start = value.start_byte();
            out.push(arg);
        }
        out
    }

    fn record_catch_blocks(&mut self, node: Node) {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        for pair in children.windows(2) {
            if pair[0].kind() == "catch_clause" && pair[1].kind() == "block" {
                self.facts.catch_blocks.push(identifiers_in(pair[1], self.src));
            }
        }
    }

    fn record_index(&mut self, node: Node) {
        let (Some(object), Some(index)) = (node.child_by_field_name("object"), node.child_by_field_name("index"))
        else {
            return;
        };
        let read = IndexRead {
            object: self.text(object),
            index: self.text(index),
            line: node.start_position().row + 1,
        };
        self.facts.index_reads.push(read);
    }

    fn record_signature(&mut self, node: Node) {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let returns = node
            .child_by_field_name("return_type")
            .and_then(|t| find(t, "type_identifier"))
            .map(|t| self.text(t))
            .unwrap_or_default();
        self.facts.signatures.push(Signature {
            name: self.text(name),
            returns,
            line: node.start_position().row + 1,
        });
    }

    fn record_binding(&mut self, node: Node) {
        let (name, value) = match node.kind() {
            "named_argument" => (
                find(node, "label")
                    .and_then(|l| find(l, "identifier"))
                    .map(|i| self.text(i)),
                node.named_child(node.named_child_count().saturating_sub(1) as u32),
            ),
            "assignment_expression" => (
                node.child_by_field_name("left").map(|l| {
                    l.child_by_field_name("property")
                        .map(|p| self.text(p))
                        .unwrap_or_else(|| self.text(l))
                }),
                node.child_by_field_name("right"),
            ),
            _ => (
                node.child_by_field_name("name").map(|n| self.text(n)),
                node.child_by_field_name("value"),
            ),
        };
        if let (Some(name), Some(value)) = (name, value) {
            let binding = Binding {
                name,
                identifiers: identifiers_in(value, self.src),
                line: node.start_position().row + 1,
            };
            self.facts.bindings.push(binding);
        }
    }
}

/// The first descendant (depth-first, including `node`) of the given kind.
fn find<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<Node<'t>> = node.children(&mut cursor).collect();
    children.into_iter().find_map(|child| find(child, kind))
}

fn children_by_field<'t>(node: Node<'t>, field: &str) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor).collect()
}

fn identifiers_in(node: Node, src: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if matches!(
            current.kind(),
            "identifier" | "type_identifier" | "identifier_dollar_escaped"
        ) {
            out.insert(current.utf8_text(src).unwrap_or_default().to_string());
        }
        let mut cursor = current.walk();
        stack.extend(current.children(&mut cursor));
    }
    out
}

/// A string literal's content, or `None` when it interpolates (its value is not known statically).
fn string_content(node: Node, src: &[u8]) -> Option<String> {
    let mut content = String::new();
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        match current.kind() {
            "template_substitution" => return None,
            kind if kind.starts_with("template_chars") => {
                content.insert_str(0, current.utf8_text(src).unwrap_or_default());
            }
            _ => {
                let mut cursor = current.walk();
                stack.extend(current.children(&mut cursor));
            }
        }
    }
    Some(content)
}
