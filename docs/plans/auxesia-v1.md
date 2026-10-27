# Auxesia v1 implementation plan

Build a local Rust documentation server with Axum, Tokio, Comrak, and plain CSS based on sw.
The approved conversation supplies the detailed specification.

## Stages

1. Parse configuration and frontmatter. Aggregate errors across all eligible pages.
2. Render trusted Markdown and HTML. Serve HTML or exact Markdown through Accept negotiation.
3. Discover standalone and bundled skills. Generate llms.txt and reproducible .skill ZIP downloads.
4. Capture immutable site snapshots. Retain the last valid snapshot after errors and show developer-mode diagnostics.
5. Validate routing, archives, concurrent refreshes, and failure recovery. Write author documentation.

## Contracts

- Read `.auxesia.yaml` from the docs root. Support title, subtitle, stylesheet, ignore, and llms.header.
- Require page title or skill name. Warn about missing descriptions. Preserve original source bytes.
- Hide index:false pages from discovery only. Ignore paths completely after a successful publication.
- Render bundles at /skills/slug.md. Include supporting files in /skills/slug.skill.
- Validate every eligible page before rejecting a candidate. Publish configuration, content, and archives together.
- Use a shared refresh on requests, with fingerprints before and after capture. Retry instability once.
- Show errors on the next HTML page load in developer mode. Keep raw responses unchanged.
- Keep root-relative URLs. Defer Git checkout management, webhooks, MCP, live updates, and static export.

## Validation

Run cargo fmt --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
Use fixtures inspired by objgit without changing the objgit repository.
