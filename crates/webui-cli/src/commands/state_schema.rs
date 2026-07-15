// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use anyhow::{Context, Result};
use clap::Args;
use expand_tilde::expand_tilde;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use webui_protocol::condition_expr;
use webui_protocol::web_ui_fragment;
use webui_protocol::{
    ComparisonOperator, ConditionExpr, FragmentList, WebUIFragment, WebUIProtocol,
};

#[derive(Args)]
pub struct StateSchemaArgs {
    /// Path to a protocol.bin file
    pub file: PathBuf,

    /// Entry fragment to analyze
    #[arg(long, default_value = "index.html")]
    pub entry: String,

    /// Schema title
    #[arg(long, default_value = "WebUIState")]
    pub title: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InferredKind {
    String,
    Boolean,
    Number,
    Object,
}

#[derive(Default)]
struct Node {
    kind: Option<InferredKind>,
    children: BTreeMap<String, Node>,
    array_item: Option<Box<Node>>,
}

#[derive(Clone)]
struct LoopScope {
    item: String,
    collection: String,
}

pub fn execute(args: &StateSchemaArgs) -> Result<()> {
    let input_file = expand_tilde(&args.file)
        .with_context(|| format!("Failed to expand input path: {}", args.file.display()))?
        .into_owned();

    let protocol = WebUIProtocol::from_protobuf_file(&input_file)
        .with_context(|| format!("Failed to read protocol {}", input_file.display()))?;

    let schema = generate_schema(&protocol, &args.entry, &args.title)
        .with_context(|| format!("Failed to generate schema for entry {}", args.entry))?;
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

fn generate_schema(protocol: &WebUIProtocol, entry: &str, title: &str) -> Result<Value> {
    let mut root = Node::default();
    let mut visited = BTreeSet::new();
    walk_fragment_list(protocol, entry, &[], &mut root, &mut visited)?;
    let mut schema = node_to_schema(&root);
    if let Some(object) = schema.as_object_mut() {
        object.insert(
            "$schema".to_string(),
            Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
        );
        object.insert("title".to_string(), Value::String(title.to_string()));
    }

    Ok(schema)
}

fn walk_fragment_list(
    protocol: &WebUIProtocol,
    fragment_id: &str,
    scopes: &[LoopScope],
    root: &mut Node,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    if !visited.insert(fragment_id.to_string()) {
        return Ok(());
    }

    let Some(FragmentList { fragments }) = protocol.fragments.get(fragment_id) else {
        return Ok(());
    };

    for fragment in fragments {
        walk_fragment(protocol, fragment, scopes, root, visited)?;
    }

    visited.remove(fragment_id);
    Ok(())
}

fn walk_fragment(
    protocol: &WebUIProtocol,
    fragment: &WebUIFragment,
    scopes: &[LoopScope],
    root: &mut Node,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    match fragment.fragment.as_ref() {
        Some(web_ui_fragment::Fragment::Signal(signal)) => {
            add_path(root, scopes, &signal.value, InferredKind::String);
        }
        Some(web_ui_fragment::Fragment::ForLoop(for_loop)) => {
            add_array_path(root, scopes, &for_loop.collection);
            let mut next_scopes = scopes.to_vec();
            next_scopes.push(LoopScope {
                item: for_loop.item.clone(),
                collection: for_loop.collection.clone(),
            });
            walk_fragment_list(protocol, &for_loop.fragment_id, &next_scopes, root, visited)?;
        }
        Some(web_ui_fragment::Fragment::IfCond(if_cond)) => {
            if let Some(condition) = &if_cond.condition {
                add_condition_paths(root, scopes, condition);
            }
            walk_fragment_list(protocol, &if_cond.fragment_id, scopes, root, visited)?;
        }
        Some(web_ui_fragment::Fragment::Attribute(attribute)) => {
            if !attribute.value.is_empty() {
                add_path(root, scopes, &attribute.value, InferredKind::String);
            }
            if !attribute.template.is_empty() {
                walk_fragment_list(protocol, &attribute.template, scopes, root, visited)?;
            }
            if let Some(condition) = &attribute.condition_tree {
                add_condition_paths(root, scopes, condition);
            }
        }
        Some(web_ui_fragment::Fragment::Component(component)) => {
            walk_fragment_list(protocol, &component.fragment_id, scopes, root, visited)?;
        }
        Some(web_ui_fragment::Fragment::Route(route)) => {
            walk_fragment_list(protocol, &route.fragment_id, scopes, root, visited)?;
            for child in &route.children {
                walk_fragment(
                    protocol,
                    &WebUIFragment {
                        fragment: Some(web_ui_fragment::Fragment::Route(child.clone())),
                    },
                    scopes,
                    root,
                    visited,
                )?;
            }
        }
        Some(web_ui_fragment::Fragment::Raw(_))
        | Some(web_ui_fragment::Fragment::Plugin(_))
        | Some(web_ui_fragment::Fragment::Outlet(_))
        | None => {}
    }

    Ok(())
}

fn add_condition_paths(root: &mut Node, scopes: &[LoopScope], condition: &ConditionExpr) {
    match condition.expr.as_ref() {
        Some(condition_expr::Expr::Identifier(identifier)) => {
            add_path(root, scopes, &identifier.value, InferredKind::Boolean);
        }
        Some(condition_expr::Expr::Predicate(predicate)) => {
            let kind = match ComparisonOperator::try_from(predicate.operator).ok() {
                Some(ComparisonOperator::GreaterThan)
                | Some(ComparisonOperator::LessThan)
                | Some(ComparisonOperator::GreaterThanOrEqual)
                | Some(ComparisonOperator::LessThanOrEqual) => InferredKind::Number,
                _ => InferredKind::String,
            };
            add_path(root, scopes, &predicate.left, kind);
            add_path(root, scopes, &predicate.right, kind);
        }
        Some(condition_expr::Expr::Not(not)) => {
            if let Some(inner) = &not.condition {
                add_condition_paths(root, scopes, inner);
            }
        }
        Some(condition_expr::Expr::Compound(compound)) => {
            if let Some(left) = &compound.left {
                add_condition_paths(root, scopes, left);
            }
            if let Some(right) = &compound.right {
                add_condition_paths(root, scopes, right);
            }
        }
        None => {}
    }
}

fn add_path(root: &mut Node, scopes: &[LoopScope], path: &str, kind: InferredKind) {
    if path.is_empty() || is_literal(path) {
        return;
    }

    let resolved_path = resolve_scoped_path(scopes, path);
    let parts: Vec<&str> = resolved_path.split('.').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return;
    }

    add_parts(root, &parts, kind);
}

fn add_array_path(root: &mut Node, scopes: &[LoopScope], path: &str) {
    let resolved_path = resolve_scoped_path(scopes, path);
    let parts: Vec<&str> = resolved_path.split('.').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return;
    }

    let node = get_or_create_node(root, &parts);
    node.array_item.get_or_insert_with(|| Box::new(Node::default()));
}

fn add_parts(root: &mut Node, parts: &[&str], kind: InferredKind) {
    if parts.last() == Some(&"length") {
        let parent_parts = &parts[..parts.len().saturating_sub(1)];
        if !parent_parts.is_empty() {
            let node = get_or_create_node(root, parent_parts);
            node.kind = Some(InferredKind::Object);
        }
        return;
    }

    let node = get_or_create_node(root, parts);
    node.kind = Some(merge_kind(node.kind, kind));
}

fn get_or_create_node<'a>(root: &'a mut Node, parts: &[&str]) -> &'a mut Node {
    let mut current = root;
    for part in parts {
        if let Some(array_name) = part.strip_suffix("[]") {
            current.kind.get_or_insert(InferredKind::Object);
            current = current.children.entry(array_name.to_string()).or_default();
            current = current.array_item.get_or_insert_with(|| Box::new(Node::default()));
            continue;
        }

        current.kind.get_or_insert(InferredKind::Object);
        current = current.children.entry((*part).to_string()).or_default();
    }
    current
}

fn merge_kind(existing: Option<InferredKind>, incoming: InferredKind) -> InferredKind {
    match (existing, incoming) {
        (Some(InferredKind::Object), _) | (_, InferredKind::Object) => InferredKind::Object,
        (Some(InferredKind::Number), _) | (_, InferredKind::Number) => InferredKind::Number,
        (Some(InferredKind::Boolean), _) | (_, InferredKind::Boolean) => InferredKind::Boolean,
        _ => InferredKind::String,
    }
}

fn resolve_scoped_path(scopes: &[LoopScope], path: &str) -> String {
    for scope in scopes.iter().rev() {
        if path == scope.item {
            return format!("{}[]", scope.collection);
        }

        let prefix = format!("{}.", scope.item);
        if let Some(rest) = path.strip_prefix(&prefix) {
            return format!("{}[].{}", scope.collection, rest);
        }
    }

    path.to_string()
}

fn is_literal(value: &str) -> bool {
    value == "true"
        || value == "false"
        || value == "null"
        || value.parse::<f64>().is_ok()
        || ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
}

fn node_to_schema(node: &Node) -> Value {
    if let Some(item) = &node.array_item {
        return json!({
            "type": "array",
            "items": node_to_schema(item)
        });
    }

    if !node.children.is_empty() || node.kind == Some(InferredKind::Object) {
        let mut properties = Map::new();
        let mut required = Vec::with_capacity(node.children.len());
        for (name, child) in &node.children {
            let clean_name = name.trim_end_matches("[]").to_string();
            properties.insert(clean_name.clone(), node_to_schema(child));
            required.push(Value::String(clean_name));
        }

        return json!({
            "type": "object",
            "required": required,
            "properties": properties
        });
    }

    match node.kind.unwrap_or(InferredKind::String) {
        InferredKind::Boolean => json!({ "type": "boolean" }),
        InferredKind::Number => json!({ "type": "number" }),
        InferredKind::String | InferredKind::Object => json!({ "type": "string" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use webui_protocol::{ConditionExpr, FragmentList, WebUIFragment};

    #[test]
    fn generate_schema_infers_loop_item_shape() {
        let protocol = protocol_with_fragments([
            (
                "index.html",
                vec![WebUIFragment::for_loop("item", "filesList.items", "for-1")],
            ),
            (
                "for-1",
                vec![
                    WebUIFragment::signal("item.name", false),
                    WebUIFragment::signal("item.source", false),
                    WebUIFragment::if_cond(ConditionExpr::identifier("item.shared"), "if-1"),
                ],
            ),
            ("if-1", vec![WebUIFragment::raw("shared")]),
        ]);

        let schema = generate_schema(&protocol, "index.html", "FilesState").unwrap();
        assert_eq!(schema["properties"]["filesList"]["properties"]["items"]["type"], "array");
        assert_eq!(
            schema["properties"]["filesList"]["properties"]["items"]["items"]["properties"]["name"]["type"],
            "string"
        );
        assert_eq!(
            schema["properties"]["filesList"]["properties"]["items"]["items"]["properties"]["shared"]["type"],
            "boolean"
        );
    }

    #[test]
    fn generate_schema_infers_number_from_ordered_predicate() {
        let protocol = protocol_with_fragments([(
            "index.html",
            vec![WebUIFragment::if_cond(
                ConditionExpr::predicate("count", ComparisonOperator::GreaterThan, "0"),
                "if-1",
            )],
        )]);

        let schema = generate_schema(&protocol, "index.html", "CountState").unwrap();
        assert_eq!(schema["properties"]["count"]["type"], "number");
    }

    fn protocol_with_fragments<const N: usize>(
        fragments: [(&str, Vec<WebUIFragment>); N],
    ) -> WebUIProtocol {
        let map: HashMap<String, FragmentList> = fragments
            .into_iter()
            .map(|(name, fragments)| (name.to_string(), FragmentList { fragments }))
            .collect();
        WebUIProtocol::new(map)
    }
}
