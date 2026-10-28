---
navbar: Skills
title: Skills
description: Publish agent instructions as Markdown pages and downloadable skill archives.
---

# Skills

Auxesia publishes instructions for agents. It never runs a skill or a script from a skill bundle.
The HTML pages and `/llms.txt` explain how agents can find pages and download skills.
Agents can also read skills with tools through the [MCP endpoint](/reference/mcp.md).
Each skill page has a download button above its instructions.

## Source layout

A skill can use one Markdown file or a folder with an entrypoint named `SKILL.md`.
Both forms need YAML frontmatter with a non-empty `name` string.
A `description` string gives agents a summary. Auxesia reports a warning without it.

```text
docs/
  skills/
    inspect-docs.md
    serve-auxesia/
      SKILL.md
      references/
        checks.md
```

The standalone file is useful for instructions that need no other files.
Its archive contains only `inspect-docs/SKILL.md`, with the original source bytes.

The folder form includes supporting files, such as reference text, scripts, and images.
Only the entrypoint becomes a rendered page.
The entrypoint name is case-sensitive on all operating systems.
A folder with `skill.md` but no `SKILL.md` is not a bundle. Its Markdown files are ordinary pages that need a `title` string.
Supporting Markdown files do not need frontmatter because Auxesia serves them as raw files.

The `name` field supplies the display name. The filename or folder name supplies the URL slug and archive folder name.
These names can differ, but a matching name makes the archive easier to use.

```yaml
---
name: serve-auxesia
description: Run Auxesia from its Docker image and inspect its documentation responses.
---
```

## URLs and responses

With `docs` as the content root, the sample bundle has these routes:

| Route | Response |
| --- | --- |
| `/skills/serve-auxesia` | HTML page, or original Markdown with `Accept: text/markdown` |
| `/skills/serve-auxesia.skill` | Downloadable ZIP archive |
| `/skills/serve-auxesia/SKILL.md` | Redirect to the canonical `/skills/serve-auxesia` page |
| `/skills/serve-auxesia/` | Redirect to the canonical page |
| `/skills/serve-auxesia.md` | Redirect to the canonical page |
| `/skills/serve-auxesia/references/checks.md` | Raw supporting Markdown |

A standalone `docs/skills/inspect-docs.md` uses `/skills/inspect-docs` and `/skills/inspect-docs.skill`.
The `.md` URL redirects to the canonical page for both forms. Folder aliases apply only to bundles.
Do not create both forms with the same slug. Their canonical routes conflict.

The files `skills/index.md` and `skills/README.md` are not skills.
They supply the page for `/skills/`, as in other folders, and need a `title` string.

Bundle HTML resolves relative links and images against the bundle folder.
For example, `references/checks.md` becomes `/skills/serve-auxesia/references/checks.md`.
This also applies to `href` and `src` attributes in raw HTML.
Fragment links such as `#install`, absolute paths, and URLs with a scheme do not change.
Markdown responses keep the original source, frontmatter, and relative links.
They do not include the HTML download link.

## Archive contents

The `.skill` extension identifies a ZIP archive. Every entry uses one top-level folder named after the slug.

```text
serve-auxesia/
  SKILL.md
  references/
    checks.md
```

Auxesia includes eligible files from the bundle folder and its subfolders.
Dotfiles and ignored files do not enter the archive.
Auxesia rejects symlinks and other non-regular files.
The archive preserves source bytes.

Archive output is deterministic for the same slug, file paths, bytes, and executable flags.
Entries use sorted paths and a fixed ZIP timestamp.
Executable files use mode `0755`. Other files use mode `0644`.
Auxesia retains the executable flag, rather than every original permission bit.

## Links inside skills

Local Markdown links must refer to files inside the archive.
For example, `[Response checks](references/checks.md)` works in the sample bundle.
An absolute site link or a link outside the bundle produces a warning.
A standalone skill cannot include a supporting file through a relative link.

Auxesia reports link warnings without blocking publication.
External URLs do not need an archive entry.

## Discovery and exclusion

Skills appear in directory listings and `/llms.txt` by default.
With `index: false` in the entrypoint frontmatter, Auxesia excludes the skill from these discovery lists.
Its page and archive remain available through their direct URLs.

MCP clients can also find skills through the `/mcp` endpoint.
The `list_skills` tool uses the same discovery rules. See [MCP](/reference/mcp.md).

The `.auxesia.yaml` file can exclude files with `ignore` patterns.
An ignored supporting file disappears from the raw routes and the archive.
An ignored bundle entrypoint excludes the entire bundle, including its supporting files and download.

```yaml
ignore:
  - skills/private/SKILL.md
```

## Example bundle

The repository includes a [serve-auxesia skill](/skills/serve-auxesia).
It runs `ghcr.io/tigrisdata/auxesia`, serves the bundled documentation or a mounted folder, and describes response checks.
The [download](/skills/serve-auxesia.skill) includes its supporting reference file.
