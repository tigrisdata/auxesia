---
navbar: Home
title: Auxesia
description: Serve Markdown documentation and downloadable skills from a local folder.
---

# Auxesia

Auxesia is a documentation server in the vein of something like [Docusaurus](https://docusaurus.io/) or [Mintlify](https://www.mintlify.com/) but aimed at experienced developers that prefer shoving their docs into a folder of markdown files and being done with it. Auxesia is written in Rust and uses [comrak](https://docs.rs/comrak/latest/comrak/) to parse markdown.

Auxesia is also designed to be SEO and AEO compliant so that humans and agents both have stellar experiences reading your documentation and then getting back to what really matters: changing the world through business to business software as a service applications.

In an ideal world, the Auxesia documentation workflow looks like this:

1. Implement cool features in your project.
2. Edit files in `docs/whatever.md`.
3. Hit save.
4. There is no step four.

## SEO/AEO automations

Auxesia handles the following SEO/AEO tasks for you:

- [`/llms.txt`](https://llmstxt.org/) generation to give agents a high level idea on how to get started.
- [`Accept: text/markdown`](https://acceptmarkdown.com/) to give agents a plaintext version of your documentation.

## Guides

Want to get started? Look here first!

- [Get started](./getting-started.md): Run this site, install Auxesia, and create a documentation folder.
- [Write pages](./authoring.md): Add frontmatter, links, assets, and directory pages.
- [Deploy with Docker](./deployment.md): Build an image and serve a mounted documentation folder.
- [MCP clients](./mcp.md): Add this website's MCP server to your AI agent and why you would want to do that.

## Skills

As Auxesia is an agent-native documentation platform, there is first-class support for agent skills. Here are the skills that are defined in this repo:

- [Serve Auxesia skill](./skills/serve-auxesia): Read the example skill and download its archive.

## Reference

- [Configuration](./reference/configuration.md): Set the site title, stylesheet, ignore patterns, and agent introduction.
- [HTTP](./reference/http.md): Use page URLs, content negotiation, and generated routes.
- [Operations](./reference/operations.md): Run the server and understand update errors.
- [Skills](./reference/skills.md): Publish individual skills and skill bundles.
