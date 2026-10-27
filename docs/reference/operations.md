---
title: Operations reference
description: Understand startup errors, request refreshes, snapshot publication, and deployment limits.
---

# Operations reference

Auxesia serves a local documentation directory from an in-memory snapshot.
See [Getting started](/getting-started.md) for installation and command syntax.

## Startup

At startup, Auxesia reads the configuration and all eligible files.
It parses every eligible page and collects errors across those pages.
The command reports collected warnings and errors to standard error.
Errors prevent the server from starting.
Warnings alone do not prevent startup.

Source errors include invalid configuration, unreadable files, invalid page frontmatter, symbolic links, and conflicting routes.
Missing descriptions produce warnings.
Skill links to files outside their archives also produce warnings.

The default listener is `127.0.0.1:8080`.
The `--listen` argument selects another socket address.
Ctrl-C requests graceful shutdown.

## Request refreshes

Each `GET` or `HEAD` request triggers a scan and read of the eligible source tree.
Concurrent requests share an active refresh and wait for its result.
The refresh continues after the initiating request ends.
Unchanged input reuses the current snapshot.
Unchanged skill contents reuse their archive bytes after unrelated edits.

Before publication, Auxesia captures the source tree again and compares its fingerprint with the first capture.
If the fingerprints differ, Auxesia retries once with the newer capture.
If the second attempt also changes, Auxesia rejects that candidate.
This comparison detects changes during the build but does not lock the source directory.

Auxesia publishes a complete valid snapshot in one atomic state change.
The snapshot includes the configuration, pages, static assets, discovery files, and skill archives.
Each request uses one snapshot for its response.
Requests around a publication can receive different valid snapshots.

## Errors after startup

If an update contains errors, Auxesia retains the last valid whole snapshot.
The server reports new errors to standard error.
It does not publish the valid portions of an invalid candidate.
The previous configuration, assets, routes, and archives remain available together.

For example, an ignore change does not remove a published route until the complete candidate passes all checks.
An invalid stylesheet or page elsewhere in the tree prevents that publication.
After the source changes again, a request retries the update.

With `--dev`, the next HTML page response includes an error toast for an unsuccessful update.
The toast lists the errors and asks the reader to fix them and reload.
Markdown responses, static responses, and archives do not include this toast.
Auxesia does not push updates to an open browser page.
A page reload or another request triggers the next refresh.

## Source and trust limits

Auxesia renders trusted Markdown with raw HTML enabled.
Authors can include HTML and scripts in page bodies.
The server does not sanitize those bodies.
The documentation root must contain content that you trust.

Auxesia reads regular files and directories.
Symbolic links and special files produce errors unless exclusion rules omit their paths first.
The `.auxesia.yaml` file also cannot be a symbolic link.
Filenames must use UTF-8.
Dot-prefixed files and directories are excluded, except for the separately read configuration file.

The server captures file contents in memory and reads the source tree on requests.
It targets modest documentation sites with local source files.
Large files and large trees increase memory use and refresh work.
Skill contents are instructions for the consumer.
Auxesia does not execute skill code.

## Deferred features

The current command serves a local directory.
It does not manage repository checkouts, Git synchronization, webhooks, or an MCP server.
Browser live updates and static export are also deferred.
The server currently has no authentication or authorization layer.

See [Configuration](/reference/configuration.md) for exclusions and [Skills](/reference/skills.md) for archive behavior.
