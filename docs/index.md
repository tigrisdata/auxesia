---
title: Auxesia
description: Serve Markdown documentation and downloadable skills from a local folder.
---
Auxesia serves Markdown documentation and downloadable skills from a local folder. Browsers receive HTML. Agents can request the original Markdown at the same URL.

Auxesia serves these pages from its own `docs` folder. The documentation is also a working example of a complete site.

## Run this site

From the repository root, run:

```sh
cargo run -- ./docs --dev
```

Open <http://127.0.0.1:8080/> in a browser.

To request this page as Markdown, run:

```sh
curl -H 'Accept: text/markdown' http://127.0.0.1:8080/
```

The Markdown response includes the original YAML frontmatter. The server also publishes a documentation and skills list at [/llms.txt](/llms.txt).

## Guides

- [Get started](/getting-started.md): Run this site, install Auxesia, and create a documentation folder.
- [Write pages](/authoring.md): Add frontmatter, links, assets, and directory pages.
- [Deploy with Docker](/deployment.md): Build an image and serve a mounted documentation folder.
- [Serve Auxesia skill](/skills/serve-auxesia.md): Read the example skill and download its archive.

## Reference

- [Configuration](/reference/configuration.md): Set the site title, stylesheet, ignore patterns, and agent introduction.
- [HTTP](/reference/http.md): Use page URLs, content negotiation, and generated routes.
- [Operations](/reference/operations.md): Run the server and understand update errors.
- [Skills](/reference/skills.md): Publish individual skills and skill bundles.
