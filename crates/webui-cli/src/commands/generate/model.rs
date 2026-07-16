// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

use super::naming::{route_type_suffix, to_pascal_case, NameAllocator};
use super::schema_input::{normalize_union, preferred_type, primitive_type, resolve_definition};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum PreferredType {
    String,
    Boolean,
    Number,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum TypeRef {
    Any,
    Never,
    Null,
    Boolean,
    Integer,
    Number,
    String,
    Array(Box<TypeRef>),
    Object(String),
    Union(Vec<TypeRef>),
    Preferred {
        accepted: Box<TypeRef>,
        preferred: PreferredType,
    },
}

#[derive(Clone, Debug)]
pub(super) struct Property {
    pub(super) json_name: String,
    pub(super) required: bool,
    pub(super) ty: TypeRef,
}

#[derive(Clone, Debug)]
pub(super) struct TypeDefinition {
    pub(super) name: String,
    pub(super) properties: Vec<Property>,
}

#[derive(Clone, Debug)]
pub(super) struct RouteBinding {
    pub(super) path: String,
    pub(super) type_name: String,
    pub(super) variant_name: String,
}

#[derive(Clone, Debug)]
pub(super) struct TypeDocument {
    pub(super) name: String,
    pub(super) definitions: Vec<TypeDefinition>,
    pub(super) roots: Vec<String>,
    pub(super) routes: Vec<RouteBinding>,
}

struct DocumentBuilder {
    names: NameAllocator,
    definitions: Vec<TypeDefinition>,
}

enum ParseTask<'a> {
    Type {
        schema: &'a Value,
        candidate: String,
    },
    FinishArray,
    FinishUnion {
        count: usize,
    },
    FinishPreferred {
        preferred: PreferredType,
    },
    FinishObject {
        definition_index: usize,
        properties: Vec<(String, bool)>,
    },
}

struct ParseState<'a> {
    tasks: Vec<ParseTask<'a>>,
    values: Vec<TypeRef>,
}

impl TypeDocument {
    pub(super) fn from_schema(schema: &Value, name_override: Option<&str>) -> Result<Self> {
        let candidate = name_override
            .or_else(|| schema.get("title").and_then(Value::as_str))
            .unwrap_or("WebUIState");
        let mut builder = DocumentBuilder::new();

        let routes = schema.get("x-webui-routes").and_then(Value::as_object);
        if let Some(routes) = routes {
            return builder.build_routed(schema, candidate, routes);
        }

        let root = builder.parse_type(schema, candidate.to_string())?;
        let TypeRef::Object(name) = root else {
            bail!("The schema root must be an object");
        };
        Ok(Self {
            name: name.clone(),
            definitions: builder.definitions,
            roots: vec![name],
            routes: Vec::new(),
        })
    }

    pub(super) fn uses_never(&self) -> bool {
        let mut pending: Vec<&TypeRef> = self
            .definitions
            .iter()
            .flat_map(|definition| definition.properties.iter().map(|property| &property.ty))
            .collect();
        while let Some(ty) = pending.pop() {
            match ty {
                TypeRef::Never => return true,
                TypeRef::Array(item) => pending.push(item),
                TypeRef::Union(branches) => pending.extend(branches),
                TypeRef::Preferred { .. } => {}
                _ => {}
            }
        }
        false
    }
}

impl DocumentBuilder {
    fn new() -> Self {
        Self {
            names: NameAllocator::new(),
            definitions: Vec::new(),
        }
    }

    fn build_routed(
        mut self,
        schema: &Value,
        candidate: &str,
        routes: &Map<String, Value>,
    ) -> Result<TypeDocument> {
        let name = self.names.allocate(candidate);
        let mut roots = Vec::with_capacity(routes.len());
        let mut bindings = Vec::with_capacity(routes.len());
        let mut variant_names = NameAllocator::new();

        for (path, schema_ref) in routes {
            let schema_ref = schema_ref
                .as_str()
                .with_context(|| format!("Route '{path}' must reference a schema"))?;
            let route_schema = resolve_definition(schema, schema_ref)
                .with_context(|| format!("Failed to resolve schema for route '{path}'"))?;
            let suffix = route_type_suffix(path);
            let route_type = self.parse_type(route_schema, format!("{name}{suffix}"))?;
            let TypeRef::Object(type_name) = route_type else {
                bail!("Route '{path}' state schema must be an object");
            };
            roots.push(type_name.clone());
            bindings.push(RouteBinding {
                path: path.clone(),
                type_name,
                variant_name: variant_names.allocate(&suffix),
            });
        }

        if bindings.is_empty() {
            bail!("x-webui-routes must contain at least one route");
        }

        Ok(TypeDocument {
            name,
            definitions: self.definitions,
            roots,
            routes: bindings,
        })
    }

    fn parse_type(&mut self, schema: &Value, candidate: String) -> Result<TypeRef> {
        let mut state = ParseState {
            tasks: vec![ParseTask::Type { schema, candidate }],
            values: Vec::new(),
        };

        while let Some(task) = state.tasks.pop() {
            match task {
                ParseTask::Type { schema, candidate } => {
                    self.queue_type(schema, candidate, &mut state)?;
                }
                ParseTask::FinishArray => {
                    let item = state
                        .values
                        .pop()
                        .context("Array item schema did not produce a type")?;
                    state.values.push(TypeRef::Array(Box::new(item)));
                }
                ParseTask::FinishUnion { count } => {
                    let start = state
                        .values
                        .len()
                        .checked_sub(count)
                        .context("Union schema did not produce all branch types")?;
                    let branches = state.values.split_off(start);
                    state.values.push(normalize_union(branches));
                }
                ParseTask::FinishPreferred { preferred } => {
                    let accepted = state
                        .values
                        .pop()
                        .context("Preferred type schema did not produce an accepted type")?;
                    state.values.push(TypeRef::Preferred {
                        accepted: Box::new(accepted),
                        preferred,
                    });
                }
                ParseTask::FinishObject {
                    definition_index,
                    properties,
                } => {
                    let start = state
                        .values
                        .len()
                        .checked_sub(properties.len())
                        .context("Object schema did not produce all property types")?;
                    let property_types = state.values.split_off(start);
                    let definition = self
                        .definitions
                        .get_mut(definition_index)
                        .context("Object definition slot was not found")?;
                    definition.properties = properties
                        .into_iter()
                        .zip(property_types)
                        .map(|((json_name, required), ty)| Property {
                            json_name,
                            required,
                            ty,
                        })
                        .collect();
                    state.values.push(TypeRef::Object(definition.name.clone()));
                }
            }
        }

        if state.values.len() != 1 {
            bail!(
                "Schema produced {} root types instead of one",
                state.values.len()
            );
        }
        state
            .values
            .pop()
            .context("Schema did not produce a root type")
    }

    fn queue_type<'a>(
        &mut self,
        schema: &'a Value,
        candidate: String,
        state: &mut ParseState<'a>,
    ) -> Result<()> {
        match schema {
            Value::Bool(true) => {
                state.values.push(TypeRef::Any);
                return Ok(());
            }
            Value::Bool(false) => {
                state.values.push(TypeRef::Never);
                return Ok(());
            }
            Value::Object(_) => {}
            _ => bail!("A type schema must be a JSON object or boolean"),
        }

        if schema.get("$ref").is_some() {
            bail!("Nested $ref schemas are not supported yet");
        }

        if let Some(preferred) = preferred_type(schema)? {
            state.tasks.push(ParseTask::FinishPreferred { preferred });
        }

        if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
            state.tasks.push(ParseTask::FinishUnion {
                count: branches.len(),
            });
            for (index, branch) in branches.iter().enumerate().rev() {
                state.tasks.push(ParseTask::Type {
                    schema: branch,
                    candidate: format!("{candidate}Option{}", index + 1),
                });
            }
            return Ok(());
        }

        match schema.get("type") {
            Some(Value::String(kind)) => self.queue_named_type(kind, schema, candidate, state),
            Some(Value::Array(kinds)) => {
                let mut branches = Vec::with_capacity(kinds.len());
                for kind in kinds {
                    let kind = kind
                        .as_str()
                        .context("Schema type arrays must contain strings")?;
                    branches.push(primitive_type(kind)?);
                }
                state.values.push(normalize_union(branches));
                Ok(())
            }
            Some(_) => bail!("Schema type must be a string or array of strings"),
            None if schema.get("properties").is_some() => {
                self.queue_object(schema, candidate, state)
            }
            None => {
                state.values.push(TypeRef::Any);
                Ok(())
            }
        }
    }

    fn queue_named_type<'a>(
        &mut self,
        kind: &str,
        schema: &'a Value,
        candidate: String,
        state: &mut ParseState<'a>,
    ) -> Result<()> {
        match kind {
            "object" => self.queue_object(schema, candidate, state),
            "array" => {
                state.tasks.push(ParseTask::FinishArray);
                match schema.get("items") {
                    Some(items) => state.tasks.push(ParseTask::Type {
                        schema: items,
                        candidate: format!("{candidate}Item"),
                    }),
                    None => state.values.push(TypeRef::Any),
                }
                Ok(())
            }
            _ => {
                state.values.push(primitive_type(kind)?);
                Ok(())
            }
        }
    }

    fn queue_object<'a>(
        &mut self,
        schema: &'a Value,
        candidate: String,
        state: &mut ParseState<'a>,
    ) -> Result<()> {
        let name = self.names.allocate(&candidate);
        let definition_index = self.definitions.len();
        self.definitions.push(TypeDefinition {
            name: name.clone(),
            properties: Vec::new(),
        });

        let properties = schema.get("properties").and_then(Value::as_object);
        let required: BTreeSet<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let metadata: Vec<(String, bool)> = properties
            .into_iter()
            .flat_map(|values| values.keys())
            .map(|json_name| (json_name.clone(), required.contains(json_name.as_str())))
            .collect();

        state.tasks.push(ParseTask::FinishObject {
            definition_index,
            properties: metadata,
        });
        if let Some(properties) = properties {
            for (json_name, property_schema) in properties.iter().rev() {
                state.tasks.push(ParseTask::Type {
                    schema: property_schema,
                    candidate: format!("{name}{}", to_pascal_case(json_name)),
                });
            }
        }
        Ok(())
    }
}
