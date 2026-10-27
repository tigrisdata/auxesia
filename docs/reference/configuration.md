---
title: Configuration reference
description: Configure site metadata, custom CSS, ignored paths, and page frontmatter.
---

# Configuration reference

Auxesia reads the optional `.auxesia.yaml` file from the documentation root.
The root is the directory argument to `auxesia`.
See [Getting started](/getting-started.md) for the command syntax.

```yaml
title: Engineering handbook
subtitle: Notes for our team
footer: 'Maintained by **Engineering** · [Contact](/contact.md)'
stylesheet: assets/custom.css
ignore:
  - drafts
  - "**/*.tmp"
llms:
  header: Start with the installation guide.
```

## Supported fields

| Field | Type | Default | Behavior |
| --- | --- | --- | --- |
| `title` | String | Name of the root directory | Site title in the HTML header, metadata, and `/llms.txt`. An empty or whitespace-only title is an error. |
| `subtitle` | String | Empty string | Text beside the site title in the HTML header. |
| `footer` | Optional string | Attribution shown below | Markdown rendered in the footer. An empty or whitespace-only string hides the footer. |
| `stylesheet` | Optional string | None | Path to a custom stylesheet, relative to the root. |
| `ignore` | List of strings | Empty list | Glob patterns that exclude paths from the site. |
| `llms` | Object | Object with an empty `header` | Configuration for `/llms.txt`. |
| `llms.header` | String | Empty string | Text after the site title in `/llms.txt`, before the generated guidance. |

Unknown fields are errors at both the top level and inside `llms`.
Invalid YAML, invalid field types, and invalid glob patterns are errors.
An absent configuration file uses all defaults.

## Footer

The default footer uses this Markdown:

```markdown
Powered by [Auxesia](https://github.com/tigrisdata/auxesia), with ❤️ from the nerds at [Tigris](https://www.tigrisdata.com)
```

The `footer` field accepts Markdown, including links, emphasis, and trusted HTML.
Use root-relative links so they work on nested pages and skill pages.
Omit the field or set it to `null` to use the default text.

```yaml
footer: |
  Maintained by **Engineering**.

  [Contact](/contact.md) · [Source](https://github.com/example/project)
```

Set `footer: ''` to hide the footer. Markdown source responses do not include the footer.

## Custom stylesheet

The `stylesheet` path must name an eligible file inside the root.
The path cannot contain empty components, `.` components, `..` components, or dot-prefixed components.
Backslashes and null characters are invalid.
Absolute paths are invalid.
Ignored files and symbolic links cannot supply the stylesheet.

Auxesia includes the custom stylesheet after its bundled stylesheet at `/_auxesia/style.css`.
The normal CSS cascade determines which declarations apply.
Auxesia does not parse the custom file as CSS.

## Ignore patterns

Patterns match paths relative to the root, with `/` as the separator.
Auxesia uses `globset` glob syntax with literal separators.
A single `*` does not cross a `/` separator.
The `**` pattern can match across directories.
Patterns are case-sensitive.

Auxesia compares each path and its ancestor paths against the patterns.
A matching directory excludes all its descendants.
For example, `drafts` excludes the root directory `drafts` and all files inside it.
The pattern `**/*.tmp` excludes `.tmp` files throughout the tree.
Patterns do not use `.gitignore` negation rules.

Dot-prefixed files and directories are always excluded, apart from the separately read `.auxesia.yaml` file.
If a pattern excludes `skills/example/SKILL.md`, Auxesia excludes the entire `example` bundle.

Ignore changes take effect only after the complete candidate site passes all checks.
If another error prevents publication, the previous site still serves its previous paths.
See [Operations](/reference/operations.md) for snapshot behavior.

## Page frontmatter

Every documentation page needs YAML frontmatter with a non-empty `title` string.
Skill entrypoints need a non-empty `name` string instead.

```markdown
---
title: Installation
description: Install the command and start a local server.
index: true
---

# Installation
```

| Field | Type | Default | Behavior |
| --- | --- | --- | --- |
| `title` | String | Required for documentation | Title in the HTML page, navigation, and discovery entries. |
| `name` | String | Required for skill entrypoints | Skill title in the HTML page, navigation, and discovery entries. |
| `description` | Optional string | None | HTML description metadata and text in `/llms.txt`. |
| `index` | Boolean | `true` | Include the page in directory navigation and `/llms.txt`. |

Missing, null, empty, or whitespace-only descriptions produce warnings.
They do not prevent publication.
The `title`, `name`, and `description` fields accept strings or null, but required titles and names cannot be null.
Unknown frontmatter fields are accepted.

The `index: false` value hides a page from discovery.
Its direct URL remains available.
An `index.md` or `README.md` page can still supply its directory route.
See [Authoring](/authoring.md) and [Skills](/reference/skills.md) for page and skill layouts.
