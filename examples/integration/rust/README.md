# WebUI Rust Example

Minimal example showing how to use WebUI as a Rust library to render a pre-built protocol with state data.

## Prerequisites

Build the hello-world app first:

```bash
cargo run -p microsoft-webui-cli -- build ../../app/hello-world/templates --out ../../app/hello-world/dist
```

## Usage

```bash
cargo run -- ../../app/hello-world/dist/protocol.bin ../../app/hello-world/data/state.json
```

This loads `protocol.bin`, passes the state from `state.json`, and prints the rendered HTML to stdout.

## Generated typed state

This example also includes a small typed fixture under `app/` and
`data/state.json`.

Regenerate its protocol, schema, and Rust state types:

```bash
cargo run -p microsoft-webui-cli -- build ./app \
  --out ./dist/typed.bin \
  --emit-schema

cargo run -p microsoft-webui-cli -- generate rust \
  ./dist/typed.state.schema.json \
  --name RustExampleState \
  --out ./src/generated_state.rs
```

Render through the generated `RustExampleState` DTO:

```bash
cargo run -- ./dist/typed.bin ./data/state.json --typed
```
