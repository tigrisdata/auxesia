---
navbar: Write pages
title: Write pages
description: Write Markdown pages with frontmatter, links, directory pages, and local assets.
---

# Write pages

Auxesia reads markdown files and converts them to HTML. To create a page, create a file such as `docs/contribution-guide.md`:

```markdown
---
title: Contribution Guide
navbar: Contribution Guide
description: Information on how to contribute to the Auxesia project.
---

# Contribution Guide
```

When Auxesia compiles that page, it will be served at `/contribution-guide`.

Then write your documentation as markdown like normal. Auxesia supports [GitHub Flavored Markdown](https://github.github.com/gfm/) including tables, task lists, strikethrough text, automatic links, and heading IDs.

## Linking pages

Use normal markdown links to connect pages:

```markdown
[Project guide](guide.md)
[HTTP reference](reference/http.md)
```

Of note: root-relative links (ones that start with `/`, such as `/contribution-guide.md`) are relative to your documentation folder root. Prefer using relative links when possible.

## Static assets

Place assets inside the `static` folder in your documentation root. Then use their paths in the Markdown body:

```markdown
![System diagram](/static/images/system.png)
[Example data](/static/examples/data.json)

![Logo](/static/logo.svg)
[Example Markdown](/static/example.md)
```

For example, `docs/static/logo.svg` is available at `/static/logo.svg` when the documentation root is `docs`.

This folder does not allow the use of symbolic links. If your architecture requires the use of symbolic links for this, consider a different architecture.

## Index pages

If a directory contains an `index.md` or `README.md` file, this file is used as the index page. The front matter in such an index page is used in the navbar and across the website. When possible, prefer using `README.md` so that your documentation renders cleanly in git forge UIs.

For example:

```markdown
---
title: Agent Skills
navbar: Skills
description: Auxesia agent skills
---

...
```

If no index page is defined for a folder, Auxesia will automatically generate a minimal index for you.

## Non-indexed pages

To omit a page from the navbar and `/llms.txt`, add `index: false` to its front matter:

```yaml
---
title: Additional details
description: Details available through a direct link.
index: false
---
```

## Front matter reference

Auxesia supports the following fields in YAML front matter:

| Field         | Type           | Default                     | Purpose                                                                                                                               |
| :------------ | :------------- | :-------------------------- | :------------------------------------------------------------------------------------------------------------------------------------ |
| `title`       | String         | Required                    | The page title in HTML and discovery entries. It must contain non-whitespace text.                                                    |
| `navbar`      | String or null | The filename or folder name | Overrides the navbar label with this text.                                                                                            |
| `description` | String or null | None                        | If set, supplies Open Graph description metadata and `/llms.txt` details for agents to pattern match against the intent of your page. |
| `index`       | Boolean        | `true`                      | If true, include the page in directory navigation and `/llms.txt`. If you want to hide a page from discovery, set this to `false`.    |

## Apply edits

After an edit, reload the page or request it again. Auxesia reads the folder for each `GET` or `HEAD` request.

If an edit causes an error, Auxesia keeps the last valid site. With `--dev`, HTML pages show the update errors.

The [operations reference](/reference/operations.md) describes startup errors and recovery.
