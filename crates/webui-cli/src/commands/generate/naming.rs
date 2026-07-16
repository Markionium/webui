// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::collections::BTreeSet;

pub(super) struct NameAllocator {
    used: BTreeSet<String>,
}

impl NameAllocator {
    pub(super) fn new() -> Self {
        Self {
            used: BTreeSet::new(),
        }
    }

    pub(super) fn allocate(&mut self, candidate: &str) -> String {
        let base = to_pascal_case(candidate);
        if self.used.insert(base.clone()) {
            return base;
        }

        let mut suffix = 2_u32;
        loop {
            let name = format!("{base}{suffix}");
            if self.used.insert(name.clone()) {
                return name;
            }
            suffix += 1;
        }
    }
}

pub(super) fn to_pascal_case(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut uppercase_next = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase_next {
                output.push(character.to_ascii_uppercase());
                uppercase_next = false;
            } else {
                output.push(character);
            }
        } else {
            uppercase_next = true;
        }
    }

    normalize_identifier_start(output, "Value")
}

pub(super) fn to_snake_case(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    let mut output = String::with_capacity(value.len());
    let mut previous_was_separator = true;

    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !output.is_empty() {
                previous_was_separator = true;
            }
            continue;
        }

        let previous = index
            .checked_sub(1)
            .and_then(|i| characters.get(i))
            .copied();
        let next = characters.get(index + 1).copied();
        let word_boundary = character.is_ascii_uppercase()
            && !output.is_empty()
            && !previous_was_separator
            && (previous.is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
                || next.is_some_and(|value| value.is_ascii_lowercase()));
        if (previous_was_separator || word_boundary) && !output.is_empty() && !output.ends_with('_')
        {
            output.push('_');
        }

        output.push(character.to_ascii_lowercase());
        previous_was_separator = false;
    }

    normalize_identifier_start(output, "value")
}

pub(super) fn route_type_suffix(path: &str) -> String {
    if path == "/" {
        return "Root".to_string();
    }

    let mut suffix = String::new();
    for segment in path.trim_matches('/').split('/') {
        if let Some(name) = segment.strip_prefix(':') {
            if let Some(optional) = name.strip_suffix('?') {
                suffix.push_str("Optional");
                suffix.push_str(&to_pascal_case(optional));
            } else {
                suffix.push_str("By");
                suffix.push_str(&to_pascal_case(name));
            }
        } else if let Some(name) = segment.strip_prefix('*') {
            suffix.push_str("Splat");
            suffix.push_str(&to_pascal_case(name));
        } else {
            suffix.push_str(&to_pascal_case(segment));
        }
    }
    normalize_identifier_start(suffix, "Route")
}

pub(super) fn is_typescript_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || matches!(first, b'_' | b'$')) {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}

pub(super) fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

fn normalize_identifier_start(mut value: String, fallback: &str) -> String {
    if value.is_empty() {
        return fallback.to_string();
    }
    if value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        value.insert_str(0, fallback);
    }
    value
}
