---
title: HTTP reference
description: Understand page representations, directory routes, redirects, static files, and discovery responses.
---

# HTTP reference

Auxesia serves documentation at its source paths, including the `.md` suffix.
For example, `guides/install.md` becomes `/guides/install.md`.
A `.md` URL returns HTML by default.

## HTML and Markdown

The `Accept` header selects the page representation.
An absent or empty header selects HTML.

```sh
curl http://127.0.0.1:8080/guides/install.md
curl -H 'Accept: text/markdown' http://127.0.0.1:8080/guides/install.md
```

HTML responses use `text/html; charset=utf-8`.
Markdown responses use `text/markdown; charset=utf-8`.
For source pages, Markdown responses contain the exact original bytes, including frontmatter and line endings.
Generated directory listings return generated Markdown.
Page responses include `Vary: Accept`, including a `406` response.

For each representation, the most specific matching media range determines its quality:

1. An exact `text/html` or `text/markdown` range.
2. The `text/*` range.
3. The `*/*` range.

The highest quality wins among ranges with equal specificity.
Auxesia then compares the qualities of the two representations.
If the Markdown quality is greater than the HTML quality, Markdown wins.
A tie selects HTML.
If both qualities are zero, Auxesia returns `406 Not Acceptable`.

| `Accept` | Result |
| --- | --- |
| `*/*` | HTML |
| `text/markdown` | Original Markdown |
| `text/html;q=0.5, text/markdown;q=0.9` | Original Markdown |
| `text/html;q=0, text/*;q=1` | Markdown, because the exact HTML range excludes HTML |
| `application/json` | `406 Not Acceptable` |

Quality defaults to `1`.
Invalid qualities and qualities outside `0` through `1` count as zero.
Qualities use thousandth precision.
Media types and parameter names are case-insensitive.
Auxesia accepts an optional `charset=utf-8` parameter, including a quoted value.
Other parameters prevent that media range from matching.

## Methods and status codes

Documentation routes accept `GET` and `HEAD`.
A `HEAD` response has the same status and representation headers as `GET`, with an empty body.
Its `Content-Length` describes the corresponding `GET` body.
Other methods return `405 Method Not Allowed` with `Allow: GET, HEAD`.

Unknown routes return `404 Not Found`.
Invalid percent escapes, invalid UTF-8 paths, and unsafe path components return `400 Bad Request`.
Unsafe components include `.` and `..`, or decoded slashes, backslashes, and null characters.

The MCP routes use different method rules:

| Route | Methods | Other methods |
| --- | --- | --- |
| `/mcp` | `POST`, `GET`, `HEAD`, `DELETE` | `405` with `Allow: POST,GET,HEAD,DELETE` |
| `/mcp/sse` | `GET`, `HEAD` | `405` with `Allow: GET,HEAD` |

On these routes, a request without an `mcp-session-id` header returns `400 Bad Request`.
The only exception is a `POST` with an `initialize` message.
See [MCP](/reference/mcp.md#http-methods-and-status-codes) for all MCP status codes.

## Directory routes and redirects

For a directory route, Auxesia selects the first available page in this order:

1. `index.md`.
2. `README.md`.
3. A generated directory listing.

These filenames are case-sensitive.
The root directory route is `/`.
Other directory routes end with `/`, such as `/guides/`.
The corresponding path without `/` returns `307 Temporary Redirect`.
Redirects preserve the query string.

Skill pages use `/skills/SLUG` as their canonical URL. The `/skills/SLUG.md` URL redirects there for both standalone skills and bundles.
Bundle aliases also return `307 Temporary Redirect` to `/skills/SLUG`:

- `/skills/SLUG/`
- `/skills/SLUG/SKILL.md`

The `/skills/` listing exists for sites without skills.
See [Skills](/reference/skills.md) for archive routes and bundle layout.

## Static files and discovery

Eligible files that are not page sources retain their relative paths.
Auxesia serves their captured bytes with a MIME type based on the filename.
Unknown extensions use `application/octet-stream`.
Files inside the root `static/` folder are also static files, including Markdown and HTML.
For example, `static/logo.svg` is available at `/static/logo.svg`.
The folder and its subfolders have no directory routes or discovery entries.

Supporting Markdown files inside skill bundles are static files.
They do not use page negotiation.
The `.skill` downloads use `application/zip` and an attachment filename.

When `favicon` is configured, generated icons use `/.auxesia/gen/` URLs.
See [Favicon configuration](/reference/configuration.md#favicon) for the generated filenames and sizes.

Auxesia generates `/llms.txt` as `text/plain; charset=utf-8`.
It includes the site title, `llms.header`, agent guidance, documentation links, skill links, and archive links.
Pages with `index: false` do not appear in this file.
The source tree cannot define `llms.txt` or files beneath `_auxesia/` because these routes are reserved.
The `/mcp` and `/mcp/sse` routes are also reserved for the [MCP endpoint](/reference/mcp.md).

Root HTML contains hidden agent guidance about `/llms.txt`, Markdown requests, skills, downloads, and the MCP endpoint.
The guidance does not appear in the original Markdown response.

## URLs and caching

Generated navigation, stylesheet, discovery, and download URLs are root-relative.
Auxesia does not rewrite URLs in the Markdown body.
Relative body URLs resolve against the page URL in the browser.
A bundle page adds a base URL of `/skills/SLUG/` for its supporting files.

Every documentation response includes `Cache-Control: no-store`.
This includes static files, redirects, and errors.
MCP responses do not include this header.
The MCP event stream uses `Cache-Control: no-cache`.
See [Operations](/reference/operations.md) for refresh and snapshot behavior.
