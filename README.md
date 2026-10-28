# Auxesia

Auxesia serves Markdown documentation and downloadable skills from a local folder. Browsers receive HTML. Agents can request the original Markdown at the same URL. MCP clients can read skills with tools at `/mcp`.

Auxesia serves its own documentation from [`docs`](docs/index.md).

## Run the documentation

The source build requires Rust and Cargo with support for the Rust 2024 edition.

From the repository root, run:

```sh
cargo run -- ./docs --dev
```

Open <http://127.0.0.1:8080/> in a browser.

To request the original Markdown, run:

```sh
curl -H 'Accept: text/markdown' http://127.0.0.1:8080/
```

To read the generated documentation and skills list, run:

```sh
curl http://127.0.0.1:8080/llms.txt
```

## Install from the checkout

```sh
cargo install --path crates/auxesia
auxesia ./docs --dev
```

The default listen address is `127.0.0.1:8080`. Use `--listen` to select another address:

```sh
auxesia ./docs --listen 127.0.0.1:9000
```

## Documentation

- [Get started](docs/getting-started.md)
- [Write pages](docs/authoring.md)
- [Deploy with Docker](docs/deployment.md)
- [Configuration](docs/reference/configuration.md)
- [HTTP reference](docs/reference/http.md)
- [Operations](docs/reference/operations.md)
- [Skills](docs/reference/skills.md)
- [MCP reference](docs/reference/mcp.md)
- [Example skill](docs/skills/serve-auxesia/SKILL.md)

## Development

Run the workspace tests:

```sh
cargo test --workspace
```
