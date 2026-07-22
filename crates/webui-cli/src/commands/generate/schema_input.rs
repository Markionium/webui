// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::model::{PreferredType, TypeRef};

pub(super) fn primitive_type(kind: &str) -> Result<TypeRef> {
    match kind {
        "null" => Ok(TypeRef::Null),
        "boolean" => Ok(TypeRef::Boolean),
        "integer" => Ok(TypeRef::Integer),
        "number" => Ok(TypeRef::Number),
        "string" => Ok(TypeRef::String),
        other => bail!("Unsupported JSON Schema type '{other}'"),
    }
}

pub(super) fn preferred_type(schema: &Value) -> Result<Option<PreferredType>> {
    let Some(webui) = schema.get("x-webui") else {
        return Ok(None);
    };
    let webui = webui.as_object().context("x-webui must be an object")?;
    let Some(preferred) = webui.get("preferredType") else {
        return Ok(None);
    };
    let preferred = preferred
        .as_str()
        .context("x-webui.preferredType must be a string")?;
    match preferred {
        "string" => Ok(Some(PreferredType::String)),
        "boolean" => Ok(Some(PreferredType::Boolean)),
        "number" => Ok(Some(PreferredType::Number)),
        other => bail!("Unsupported x-webui.preferredType '{other}'"),
    }
}

pub(super) fn normalize_union(branches: Vec<TypeRef>) -> TypeRef {
    let mut flattened = Vec::new();
    for branch in branches {
        match branch {
            TypeRef::Any => return TypeRef::Any,
            TypeRef::Never => {}
            TypeRef::Union(nested) => flattened.extend(nested),
            other => flattened.push(other),
        }
    }
    flattened.sort();
    flattened.dedup();
    match flattened.len() {
        0 => TypeRef::Never,
        1 => flattened.pop().unwrap_or(TypeRef::Never),
        _ => TypeRef::Union(flattened),
    }
}

pub(super) fn resolve_definition<'a>(schema: &'a Value, schema_ref: &str) -> Result<&'a Value> {
    let encoded = schema_ref
        .strip_prefix("#/$defs/")
        .with_context(|| format!("Unsupported schema reference '{schema_ref}'"))?;
    let decoded = percent_decode(encoded)?;
    let key = decoded.replace("~1", "/").replace("~0", "~");
    schema
        .get("$defs")
        .and_then(Value::as_object)
        .and_then(|definitions| definitions.get(&key))
        .with_context(|| format!("Schema definition '{key}' was not found"))
}

fn percent_decode(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                bail!("Invalid percent encoding in schema reference");
            }
            let high = hex_value(bytes[index + 1])
                .context("Invalid percent encoding in schema reference")?;
            let low = hex_value(bytes[index + 2])
                .context("Invalid percent encoding in schema reference")?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).context("Schema reference is not valid UTF-8")
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
