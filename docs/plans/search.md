# Documentation search implementation plan

## Goal

Embed Tantivy in Auxesia. Share one index between website search and MCP.
Target collections of up to 1,000 pages without an external service.
Use Maud for HTML templates. Search runs entirely on the server, without Pagefind.

## Index

- Index all published pages, including hidden pages, skill pages, and generated directory listings.
- Exclude ignored files, assets, navigation, scripts, styles, and skill supporting files.
- Deduplicate directory aliases. Prefer the directory URL for its authored index page.
- Search titles, headings, descriptions, body text, and code examples.
- Support case-insensitive words and quoted phrases with Unicode tokenization and no stemming.
- Require all terms and phrases. Rank titles above headings, and headings above body text.
- Return one result per page with title, URL, and plain-text excerpt. Break score ties by URL.
- Build the index before snapshot publication. Reuse unchanged search content and retain the previous snapshot on failure.
- Run indexing and queries outside async worker threads.

## Website

- Add a search form to the shared shell.
- Serve HTML at `/_auxesia/search` and JSON at `/_auxesia/search.json`.
- Support `q`, `limit`, and `offset`. Default to 10 results and permit up to 50.
- Limit queries to 512 Unicode characters. Reject invalid pagination and unclosed quotes. Empty queries return no results.
- Keep forms and pagination functional without JavaScript.
- Enhance the form with a local dialog, Ctrl/Cmd+K, Escape, focus containment, and focus restoration.
- Debounce requests by 200 ms. Discard obsolete responses. Show loading, empty, and error states.
- Escape server-rendered text and use safe DOM APIs in the browser.

## MCP

- Add read-only `search_docs(query, limit?, offset?)` with results and total count.
- Add read-only `read_doc(path)` with title, canonical URL, and original Markdown.
- Accept site-relative page paths and known aliases. Reject external URLs and non-page resources.
- Preserve skill tools. Update MCP instructions and agent discovery text.

## Validation

- Cover ranking, phrases, Unicode, code, hidden and ignored content, aliases, pagination, and invalid inputs.
- Cover edits, additions, deletions, failed refreshes, index reuse, and HTTP/MCP parity.
- Exercise keyboard navigation, stale browser responses, safe text rendering, and no-JavaScript search.
- Measure startup, rebuild, query latency, and peak memory for 1,000 pages.
- Run workspace tests, formatting, clippy, and Docker build.
- Update HTTP, MCP, and operations documentation with the simple-english skill.

## Defaults

Search is always enabled and requires the running server. No persistent index, typo tolerance,
advanced query syntax, multilingual stemming, or section-level results are included.

## Validation results

The implementation uses Maud templates for the shared shell, search results, dialog, errors, download links, and favicon links.
Browser JavaScript only controls the dialog and displays server results.

The independent review found two extraction faults. Regression tests now cover both:

- Separate headings cannot form a quoted phrase through the heading index.
- Inline markup does not split a word into separate search tokens.

Chromium checks cover keyboard controls, focus restoration, stale responses, text escaping, error states, empty results, and forms without JavaScript.
The browser test is `crates/auxesia/tests/search-browser.mjs`.
It needs a separate Playwright installation. The Rust application needs no Node.js runtime.

### 1,000-page measurement

Measured on macOS arm64 with a release build on 2026-10-28.
The synthetic collection has 1,000 flat documentation pages and 3,391,670 source bytes.
Each page contains repeated storage instructions, headings, and a code example.
The generated directory listings are also indexed.

| Measurement                   | Result                           |
| ----------------------------- | -------------------------------- |
| Startup                       | 1.084 seconds                    |
| Search median                 | 3.05 ms                          |
| Search p95                    | 26.24 ms                         |
| Edit to updated search result | 1.040 seconds                    |
| Maximum resident set size     | 266,813,440 bytes, about 254 MiB |

Search measurements include request handling and the source refresh check, without network transport.
The run alternates a word query and a phrase query across 100 requests.
Peak memory covers the full test process, including site rendering, index replacement, and the fixture.
These measurements are observations, not CI timing limits.

To repeat the measurement, build the ignored test in release mode:

```sh
cargo test --release --test search --no-run
```

Run the test executable printed by Cargo with `--ignored --nocapture`.
On macOS, prefix that command with `/usr/bin/time -l` to measure peak memory.
