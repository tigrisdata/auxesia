---
navbar: Write pages
title: Write pages
description: Write Markdown pages with frontmatter, links, directory pages, and local assets.
---
# Write pages

Auxesia reads Markdown files from a local documentation folder. Each ordinary Markdown page requires YAML frontmatter with a non-empty `title` string.

## Create a page

Create a file such as `guide.md` in the documentation folder:

```markdown
---
title: Project guide
navbar: Guide
description: Learn how to use the project.
---
# Project guide

This page explains the project.
```

The file is available at `/guide.md`. The `title` supplies HTML metadata and discovery entries. The Markdown body supplies the visible page content.

The `description` supplies HTML metadata and the summary in `/llms.txt`. A missing or blank description causes a warning. It does not prevent the server from publishing the page.

Skill pages use `name` instead of `title`. The [skills reference](/reference/skills.md) describes their frontmatter and folder structure.

## Add links and assets

Use Markdown links to connect pages:

```markdown
[Project guide](/guide.md)
[HTTP reference](/reference/http.md)
```

Root-relative links keep the same target in HTML and Markdown responses. Relative links also work, but their targets depend on the current URL.

Place assets inside the documentation folder. Then use their paths in the Markdown body:

```markdown
![System diagram](/images/system.png)
[Example data](/examples/data.json)
```

Auxesia serves eligible non-Markdown files as static assets. Hidden files and ignored paths are absent from the site. Symlinks cause an error.

Put files in the root `static/` folder to serve them without page processing:

```markdown
![Logo](/static/logo.svg)
[Example Markdown](/static/example.md)
```

For example, `docs/static/logo.svg` is available at `/static/logo.svg` when the documentation root is `docs`.
Markdown files in this folder need no frontmatter and return their original bytes.
The folder has no directory pages or discovery entries. Hidden files, ignore patterns, and symlink restrictions still apply.
See [Favicon configuration](/reference/configuration.md#favicon) to generate browser icons from an image.

## Create directory pages

A directory can contain `index.md` or `README.md`. Auxesia uses `index.md` first, then `README.md`, as the page for the directory URL.

For example, `reference/index.md` supplies the page at `/reference/`. The file also remains available at `/reference/index.md`.

Without either file, Auxesia generates a directory page from the indexed pages. The root directory uses the same rules.

The sidebar shows a folder tree for the complete documentation site.
Select a folder row to expand or collapse its contents.
Select **Overview** inside a folder to open its directory page.
Select **Home** to return to the root directory.

Page labels use the page title. Skill labels use the skill name.
The optional `navbar` field replaces this label.
Missing, null, empty, or whitespace-only values use the default label.

Folder labels use the title or `navbar` value from a discoverable directory page.
Without a discoverable directory page, the label uses the folder name with spaces instead of hyphens and underscores.
Folders appear before pages. Each group sorts alphabetically by display label, ignoring case.
The page that supplies a directory appears as **Overview**, without a duplicate file link.

The sidebar highlights the current page and expands its parent folders.
Other folders start collapsed when a page loads.
On phones, the **Menu** button opens and closes the sidebar.
Long navigation lists scroll within the sidebar.

## Control discovery

To omit a page from navigation and `/llms.txt`, add `index: false`:

```yaml
---
title: Additional details
description: Details available through a direct link.
index: false
---
```

The page remains available at its URL. `index: false` does not restrict access. An explicit link can still point to the page.

To exclude files from the site, use `ignore` in `.auxesia.yaml`. The [configuration reference](/reference/configuration.md) describes ignore patterns.

## Use Markdown features

Auxesia supports tables, task lists, strikethrough text, automatic links, and heading IDs. It also permits raw HTML in Markdown bodies.

The server publishes the original source for Markdown requests. HTML responses omit the YAML frontmatter from the visible page body.

## Apply edits

After an edit, reload the page or request it again. Auxesia reads the folder for each `GET` or `HEAD` request.

If an edit causes an error, Auxesia keeps the last valid site. With `--dev`, HTML pages show the update errors.

The [operations reference](/reference/operations.md) describes startup errors and recovery.
