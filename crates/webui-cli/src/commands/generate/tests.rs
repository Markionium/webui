// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#![allow(clippy::disallowed_methods)]

use super::csharp::CSharpGenerator;
use super::model::TypeDocument;
use super::naming::to_pascal_case;
use super::rust::RustGenerator;
use super::typescript::TypeScriptGenerator;
use super::LanguageGenerator;
use crate::commands::state_schema;
use std::path::PathBuf;
use webui::{BuildOptions, Plugin};

struct ExampleCase {
    name: &'static str,
    plugin: Plugin,
    uses_theme: bool,
}

const EXAMPLES: &[ExampleCase] = &[
    ExampleCase {
        name: "calculator",
        plugin: Plugin::WebUI,
        uses_theme: false,
    },
    ExampleCase {
        name: "commerce",
        plugin: Plugin::WebUI,
        uses_theme: false,
    },
    ExampleCase {
        name: "component-assets",
        plugin: Plugin::WebUI,
        uses_theme: true,
    },
    ExampleCase {
        name: "contact-book-manager",
        plugin: Plugin::WebUI,
        uses_theme: true,
    },
    ExampleCase {
        name: "hello-world",
        plugin: Plugin::WebUI,
        uses_theme: true,
    },
    ExampleCase {
        name: "routes",
        plugin: Plugin::WebUI,
        uses_theme: false,
    },
    ExampleCase {
        name: "service-worker",
        plugin: Plugin::WebUI,
        uses_theme: false,
    },
    ExampleCase {
        name: "todo-fast",
        plugin: Plugin::FastV3,
        uses_theme: true,
    },
    ExampleCase {
        name: "todo-webui",
        plugin: Plugin::WebUI,
        uses_theme: false,
    },
];

#[test]
fn snapshots_typescript_component_types() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let output = TypeScriptGenerator.generate(&document).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn snapshots_typescript_route_types() {
    let document = routes_document();
    let output = TypeScriptGenerator.generate(&document).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn routed_example_generated_types_are_current() {
    let document = routes_document();
    let output = TypeScriptGenerator.generate(&document).unwrap();
    let checked_in = std::fs::read_to_string(
        workspace_root().join("examples/app/routes/server/src/generated/routes-state.ts"),
    )
    .unwrap();
    assert_eq!(checked_in, output);
}

#[test]
fn contact_book_generated_types_are_current() {
    let workspace = workspace_root();
    let result = webui::build(BuildOptions {
        app_dir: workspace.join("examples/app/contact-book-manager/src"),
        plugin: Some(Plugin::WebUI),
        ..BuildOptions::default()
    })
    .unwrap();
    let schema =
        state_schema::generate_schema(&result.protocol, "index.html", "ContactBookState").unwrap();
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = TypeScriptGenerator.generate(&document).unwrap();
    let checked_in = std::fs::read_to_string(
        workspace.join("examples/app/contact-book-manager/server/generated/contact-book-state.ts"),
    )
    .unwrap();
    assert_eq!(checked_in, output);
}

#[test]
fn rust_example_generated_types_are_current() {
    let workspace = workspace_root();
    let result = webui::build(BuildOptions {
        app_dir: workspace.join("examples/integration/rust/app"),
        ..BuildOptions::default()
    })
    .unwrap();
    let schema =
        state_schema::generate_schema(&result.protocol, "index.html", "RustExampleState").unwrap();
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = RustGenerator.generate(&document).unwrap();
    let checked_in =
        std::fs::read_to_string(workspace.join("examples/integration/rust/src/generated_state.rs"))
            .unwrap();
    assert_eq!(checked_in, output);
}

#[test]
fn demo_shell_generated_types_are_current() {
    let workspace = workspace_root();
    let result = webui::build(BuildOptions {
        app_dir: workspace.join("examples/demo/src"),
        plugin: Some(Plugin::WebUI),
        ..BuildOptions::default()
    })
    .unwrap();
    let schema =
        state_schema::generate_schema(&result.protocol, "index.html", "DemoShellRenderState")
            .unwrap();
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = RustGenerator.generate(&document).unwrap();
    let checked_in =
        std::fs::read_to_string(workspace.join("examples/demo/server/src/generated_state.rs"))
            .unwrap();
    assert_eq!(checked_in, output);
}

#[test]
fn snapshots_rust_component_types() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let output = RustGenerator.generate(&document).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn snapshots_rust_route_types() {
    let document = routes_document();
    let output = RustGenerator.generate(&document).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn snapshots_csharp_component_types() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let output = CSharpGenerator::new("WebUI.Generated", "public")
        .generate(&document)
        .unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn snapshots_csharp_route_types() {
    let document = routes_document();
    let output = CSharpGenerator::new("WebUI.Generated", "public")
        .generate(&document)
        .unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn root_name_override_is_shared_across_generators() {
    let schema = fixture_schema("component-scope", "IgnoredTitle");
    let document = TypeDocument::from_schema(&schema, Some("CustomState")).unwrap();
    assert_eq!(document.name, "CustomState");
    assert_eq!(document.roots, ["CustomState"]);
}

#[test]
fn csharp_visibility_is_configurable() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let output = CSharpGenerator::new("WebUI.Generated", "internal")
        .generate(&document)
        .unwrap();
    assert!(output.contains("internal sealed class ComponentScopeState"));
    assert!(output.contains("internal partial class ComponentScopeStateSerializationContext"));
}

#[test]
fn csharp_context_preserves_default_value_properties() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let output = CSharpGenerator::new("WebUI.Generated", "public")
        .generate(&document)
        .unwrap();
    assert!(output.contains("JsonIgnoreCondition.WhenWritingNull"));
    assert!(!output.contains("PropertyNameCaseInsensitive"));
}

#[test]
fn csharp_rejects_keyword_namespace_segments() {
    let document = fixture_document("component-scope", "ComponentScopeState");
    let error = CSharpGenerator::new("Contoso.class", "public")
        .generate(&document)
        .unwrap_err();
    assert!(error.to_string().contains("reserved keyword"));
}

#[test]
fn csharp_avoids_member_name_matching_its_class() {
    let schema = serde_json::json!({
        "title": "WebUIState",
        "type": "object",
        "properties": {
            "webUIState": { "type": "string" }
        },
        "required": ["webUIState"]
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = CSharpGenerator::new("WebUI.Generated", "public")
        .generate(&document)
        .unwrap();
    assert!(output.contains("public string WebUIState2"));
}

#[test]
fn csharp_imports_json_element_for_null_schema() {
    let schema = serde_json::json!({
        "title": "NullState",
        "type": "object",
        "properties": {
            "value": { "type": "null" }
        },
        "required": ["value"]
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = CSharpGenerator::new("WebUI.Generated", "public")
        .generate(&document)
        .unwrap();
    assert!(output.contains("public global::System.Text.Json.JsonElement Value"));
}

#[test]
fn rust_rejects_reserved_self_type_name() {
    let schema = serde_json::json!({
        "title": "self",
        "type": "object",
        "properties": {}
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let error = RustGenerator.generate(&document).unwrap_err();
    assert!(error.to_string().contains("reserved"));
}

#[test]
fn rust_generates_serializable_never_type() {
    let schema = serde_json::json!({
        "title": "NeverState",
        "type": "object",
        "properties": {
            "value": false
        },
        "required": ["value"]
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = RustGenerator.generate(&document).unwrap();
    assert!(output.contains("pub enum WebUINever {}"));
    assert!(output.contains("pub value: WebUINever"));
}

#[test]
fn rust_never_helper_avoids_generated_type_collision() {
    let schema = serde_json::json!({
        "title": "WebUINever",
        "type": "object",
        "properties": {
            "value": false
        },
        "required": ["value"]
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = RustGenerator.generate(&document).unwrap();
    assert!(output.contains("pub enum WebUINever2 {}"));
    assert!(output.contains("pub struct WebUINever"));
    assert!(output.contains("pub value: WebUINever2"));
}

#[test]
fn rust_renames_self_route_variant() {
    let schema = serde_json::json!({
        "title": "AppState",
        "$defs": {
            "route": {
                "type": "object",
                "properties": {}
            },
            "route.self": {
                "type": "object",
                "properties": {}
            }
        },
        "x-webui-routes": {
            "/": "#/$defs/route",
            "/self": "#/$defs/route.self"
        }
    });
    let document = TypeDocument::from_schema(&schema, None).unwrap();
    let output = RustGenerator.generate(&document).unwrap();
    assert!(output.contains("SelfRoute(AppStateSelf)"));
    assert!(!output.contains("\n    Self(AppStateSelf)"));
}

#[test]
fn invalid_preferred_type_is_rejected() {
    let schema = serde_json::json!({
        "title": "InvalidState",
        "type": "object",
        "properties": {
            "value": {
                "type": ["string", "number"],
                "x-webui": {
                    "preferredType": "date"
                }
            }
        }
    });
    let error = TypeDocument::from_schema(&schema, None).unwrap_err();
    assert!(error.to_string().contains("preferredType"));
}

#[test]
fn all_example_schemas_generate_all_languages() {
    let workspace = workspace_root();
    let theme_path = workspace.join("packages/webui-examples-theme/tokens.json");

    for example in EXAMPLES {
        let theme = example
            .uses_theme
            .then(|| webui::load_token_file(&theme_path).unwrap());
        let result = webui::build(BuildOptions {
            app_dir: workspace
                .join("examples/app")
                .join(example.name)
                .join("src"),
            plugin: Some(example.plugin),
            theme,
            ..BuildOptions::default()
        })
        .unwrap();
        let title = format!("{}State", to_pascal_case(example.name));
        let schema = state_schema::generate_schema(&result.protocol, "index.html", &title).unwrap();
        let document = TypeDocument::from_schema(&schema, None).unwrap();

        assert!(!TypeScriptGenerator.generate(&document).unwrap().is_empty());
        assert!(!RustGenerator.generate(&document).unwrap().is_empty());
        assert!(!CSharpGenerator::new("WebUI.Generated", "public")
            .generate(&document)
            .unwrap()
            .is_empty());
    }
}

fn fixture_document(fixture: &str, title: &str) -> TypeDocument {
    let schema = fixture_schema(fixture, title);
    TypeDocument::from_schema(&schema, None).unwrap()
}

fn fixture_schema(fixture: &str, title: &str) -> serde_json::Value {
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/state-schema")
        .join(fixture);
    let result = webui::build(BuildOptions {
        app_dir,
        plugin: Some(Plugin::WebUI),
        ..BuildOptions::default()
    })
    .unwrap();
    state_schema::generate_schema(&result.protocol, "index.html", title).unwrap()
}

fn routes_document() -> TypeDocument {
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/app/routes/src");
    let result = webui::build(BuildOptions {
        app_dir,
        plugin: Some(Plugin::WebUI),
        ..BuildOptions::default()
    })
    .unwrap();
    let schema =
        state_schema::generate_schema(&result.protocol, "index.html", "RoutesState").unwrap();
    TypeDocument::from_schema(&schema, None).unwrap()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
