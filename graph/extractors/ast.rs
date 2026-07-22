use tree_sitter::{Node, Point};

use super::facts::{ExtractedPosition, ExtractedRange};

pub(crate) const MAX_SIGNATURE_CHARS: usize = 240;

pub(crate) fn child_at(node: Node<'_>, index: usize) -> Option<Node<'_>> {
    let index = u32::try_from(index).ok()?;
    node.child(index)
}

pub(crate) fn first_node_of_kinds<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index)
            && let Some(found) = first_node_of_kinds(child, kinds)
        {
            return Some(found);
        }
    }

    None
}

pub(crate) fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    first_node_of_kinds(
        node,
        &[
            "identifier",
            "type_identifier",
            "field_identifier",
            "property_identifier",
            "shorthand_property_identifier",
            "constant",
            "scope_resolution",
            "simple_identifier",
            "variable_name",
        ],
    )
}

pub(crate) fn last_identifier(node: Node<'_>) -> Option<Node<'_>> {
    let mut last = is_identifier_node(node).then_some(node);

    for index in 0..node.child_count() {
        if let Some(child) = child_at(node, index)
            && let Some(found) = last_identifier(child)
        {
            last = Some(found);
        }
    }

    last
}

pub(crate) fn is_identifier_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "constant"
            | "scope_resolution"
            | "simple_identifier"
            | "variable_name"
    )
}

pub(crate) fn has_ancestor_kind(node: Node<'_>, kind: &str) -> bool {
    let mut current = node.parent();

    while let Some(node) = current {
        if node.kind() == kind {
            return true;
        }
        current = node.parent();
    }

    false
}

pub(crate) fn node_text(node: Node<'_>, text: &str) -> Option<String> {
    node.utf8_text(text.as_bytes())
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn clean_identifier_node(
    node: Node<'_>,
    text: &str,
) -> Option<(String, ExtractedRange)> {
    clean_node(node, text, false)
}

pub(crate) fn clean_named_node(node: Node<'_>, text: &str) -> Option<(String, ExtractedRange)> {
    clean_node(node, text, true)
}

fn clean_node(
    node: Node<'_>,
    text: &str,
    trim_semicolon: bool,
) -> Option<(String, ExtractedRange)> {
    let value = node_text(node, text)?;
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`');
    let value = if trim_semicolon {
        value.trim_end_matches(';')
    } else {
        value
    };

    if value.is_empty() || value.chars().any(char::is_whitespace) {
        None
    } else {
        Some((value.to_string(), range_from_node(node)))
    }
}

pub(crate) fn signature_text(node: Node<'_>, text: &str) -> Option<String> {
    node_text(node, text).map(|value| {
        value
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(MAX_SIGNATURE_CHARS)
            .collect()
    })
}

pub(crate) fn preceding_documentation(node: Node<'_>, lines: &[&str]) -> Option<String> {
    let mut row = node.start_position().row;
    let mut docs = Vec::new();

    while row > 0 {
        row -= 1;
        let line = lines.get(row)?.trim();

        if line.is_empty() {
            if docs.is_empty() {
                continue;
            }
            break;
        }

        if is_comment_line(line) {
            docs.push(clean_comment_line(line));
        } else {
            break;
        }
    }

    docs.reverse();
    let documentation = docs
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    (!documentation.is_empty()).then_some(documentation)
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//")
        || line.starts_with("///")
        || line.starts_with("//!")
        || line.starts_with('#')
        || line.starts_with('*')
        || line.starts_with("/*")
        || line.starts_with("/**")
}

fn clean_comment_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("///")
        .trim_start_matches("//!")
        .trim_start_matches("//")
        .trim_start_matches("/**")
        .trim_start_matches("/*")
        .trim_start_matches('*')
        .trim_end_matches("*/")
        .trim()
        .to_string()
}

pub(crate) fn range_from_node(node: Node<'_>) -> ExtractedRange {
    range_from_points(node.start_position(), node.end_position())
}

pub(crate) fn range_from_points(start: Point, end: Point) -> ExtractedRange {
    ExtractedRange {
        start: position_from_point(start),
        end: position_from_point(end),
    }
}

pub(crate) fn position_from_point(point: Point) -> ExtractedPosition {
    ExtractedPosition {
        line: u32::try_from(point.row).unwrap_or(u32::MAX),
        column: u32::try_from(point.column).unwrap_or(u32::MAX),
    }
}

pub(crate) fn point_in_range(
    point: ExtractedPosition,
    start: ExtractedPosition,
    end: ExtractedPosition,
) -> bool {
    point_compare(point, start) >= 0 && point_compare(point, end) <= 0
}

pub(crate) fn point_span(start: ExtractedPosition, end: ExtractedPosition) -> u64 {
    u64::from(end.line.saturating_sub(start.line)) * 100_000
        + u64::from(end.column.saturating_sub(start.column))
}

fn point_compare(left: ExtractedPosition, right: ExtractedPosition) -> i8 {
    match left.line.cmp(&right.line) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Equal => match left.column.cmp(&right.column) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        },
    }
}

pub(crate) fn clean_module_text(value: String) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .trim_matches('<')
        .trim_matches('>')
        .to_string()
}
