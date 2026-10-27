---
title: Get started
description: Run the Auxesia documentation, install the server, and serve your own pages.
---
# Get started

Auxesia serves this documentation from the repository's `docs` folder. You can run the same site locally with Cargo.

## Requirements

The source build requires Rust and Cargo with support for the Rust 2024 edition. The HTTP examples require `curl`.

## Run the documentation

1. Open a terminal in the repository root.
2. Run the server:

   ```sh
   cargo run -- ./docs --dev
   ```

3. Open <http://127.0.0.1:8080/> in a browser.

The server listens on `127.0.0.1:8080` by default. The `--dev` flag adds update errors to HTML pages.

## Request Markdown

In another terminal, request a page with `Accept: text/markdown`:

```sh
curl -i -H 'Accept: text/markdown' \
  http://127.0.0.1:8080/getting-started.md
```

The response contains the original Markdown, including its YAML frontmatter. A normal browser request receives HTML at the same URL.

To read the generated documentation and skills list, run:

```sh
curl http://127.0.0.1:8080/llms.txt
```

## Install from the checkout

From the repository root, install the local crate:

```sh
cargo install --path crates/auxesia
```

If your shell includes the Cargo binary folder in `PATH`, run the installed command:

```sh
auxesia ./docs --dev
```

## Create your own site

1. Create a folder for your documentation:

   ```sh
   mkdir my-docs
   ```

2. Create `my-docs/index.md` with this content:

   ```markdown
   ---
   title: My documentation
   description: Instructions and reference for my project.
   ---
   # My documentation

   Welcome to the project documentation.
   ```

3. Create `my-docs/.auxesia.yaml` with this content:

   ```yaml
   title: My project
   subtitle: Documentation
   ```

4. Stop the first server with `Ctrl+C`.
5. Run your site:

   ```sh
   auxesia ./my-docs --dev
   ```

6. Open <http://127.0.0.1:8080/> in a browser.

The configuration file is optional. Without it, Auxesia uses the root folder name as the site title.

## Next steps

- [Write pages](/authoring.md) with links and assets.
- [Configure the site](/reference/configuration.md).
- [Read the HTTP reference](/reference/http.md) for routes and response formats.
- [Read the operations reference](/reference/operations.md) for server behavior.
- [Publish skills](/reference/skills.md), or read the [example skill](/skills/serve-auxesia).
