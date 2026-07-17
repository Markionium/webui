// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod csharp;
mod model;
mod naming;
mod rust;
mod schema_input;
#[cfg(test)]
mod tests;
mod typescript;
mod writer;

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use expand_tilde::expand_tilde;
use model::TypeDocument;
use std::fs;
use std::path::PathBuf;

use crate::utils::output;

trait LanguageGenerator {
    fn generate(&self, document: &TypeDocument) -> Result<String>;
}

#[derive(Args)]
pub struct GenerateArgs {
    #[command(subcommand)]
    language: GenerateLanguage,
}

#[derive(Subcommand)]
enum GenerateLanguage {
    /// Generate TypeScript interfaces and route-state types
    #[command(name = "typescript")]
    TypeScript(CommonGenerateArgs),
    /// Generate serde-compatible Rust structs and route-state types
    Rust(CommonGenerateArgs),
    /// Generate C# classes and a System.Text.Json source-generation context
    #[command(name = "csharp")]
    CSharp(CSharpGenerateArgs),
}

#[derive(Args)]
struct CommonGenerateArgs {
    /// Path to a JSON Schema produced by `webui schema` or `webui build --emit-schema`
    file: PathBuf,

    /// Override the root generated type name
    #[arg(long)]
    name: Option<String>,

    /// Write generated source to a file instead of stdout
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct CSharpGenerateArgs {
    #[command(flatten)]
    common: CommonGenerateArgs,

    /// Namespace for generated C# types
    #[arg(long, default_value = "WebUI.Generated")]
    namespace: String,

    /// Visibility for generated C# types
    #[arg(long, value_enum, default_value_t = CSharpVisibility::Public)]
    visibility: CSharpVisibility,
}

#[derive(Clone, Copy, ValueEnum)]
enum CSharpVisibility {
    Public,
    Internal,
}

impl CSharpVisibility {
    fn keyword(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
        }
    }
}

pub fn execute(args: &GenerateArgs) -> Result<()> {
    run(args).inspect_err(|error| {
        output::error(error);
        eprintln!();
    })
}

fn run(args: &GenerateArgs) -> Result<()> {
    let (generated, common) = match &args.language {
        GenerateLanguage::TypeScript(common) => {
            let document = load_document(common)?;
            (typescript::TypeScriptGenerator.generate(&document)?, common)
        }
        GenerateLanguage::Rust(common) => {
            let document = load_document(common)?;
            (rust::RustGenerator.generate(&document)?, common)
        }
        GenerateLanguage::CSharp(args) => {
            let document = load_document(&args.common)?;
            (
                csharp::CSharpGenerator::new(&args.namespace, args.visibility.keyword())
                    .generate(&document)?,
                &args.common,
            )
        }
    };
    if let Some(out) = &common.out {
        let out = expand_tilde(out)
            .with_context(|| format!("Failed to expand output path: {}", out.display()))?
            .into_owned();
        if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(&out, generated).with_context(|| format!("Failed to write {}", out.display()))?;
    } else {
        print!("{generated}");
    }
    Ok(())
}

fn load_document(args: &CommonGenerateArgs) -> Result<TypeDocument> {
    let input_file = expand_tilde(&args.file)
        .with_context(|| format!("Failed to expand schema path: {}", args.file.display()))?
        .into_owned();
    let schema_text = fs::read_to_string(&input_file)
        .with_context(|| format!("Failed to read schema {}", input_file.display()))?;
    let schema: serde_json::Value = serde_json::from_str(&schema_text)
        .with_context(|| format!("Failed to parse schema {}", input_file.display()))?;
    TypeDocument::from_schema(&schema, args.name.as_deref())
        .with_context(|| format!("Failed to load schema {}", input_file.display()))
}
