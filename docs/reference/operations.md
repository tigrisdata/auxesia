---
navbar: Operations
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
Ctrl-C or `SIGTERM` requests graceful shutdown.
Open requests have 5 seconds to finish.
After this time, Auxesia stops and closes the remaining connections, such as MCP event streams.

## Request refreshes

Each `GET` or `HEAD` request with a valid path starts a refresh.
A refresh first computes a fingerprint from the metadata of the eligible source tree.
The metadata is the path, size, and modification time of each file.
On Unix, the metadata also includes the change time, inode, device, and mode.
If the fingerprint is the same as for the current snapshot, Auxesia reuses that snapshot and reads no file contents.
Unchanged skill contents reuse their archive bytes after unrelated edits.

Files that changed in the last two seconds are an exception.
For these files, the fingerprint also includes a hash of the contents.
A later edit can have the same timestamp as a recent change, so metadata alone cannot detect it.

Concurrent requests share a refresh that has not started its scan.
If a scan is in progress, a new request waits for the next refresh.
That refresh starts after the current scan ends, and all requests that arrive during the scan share it.
As a result, each response comes from a scan that started after the request arrived.
The refresh continues after the initiating request ends.
If a refresh stops before it publishes a result, the next request starts a new refresh.

When the fingerprint is different, Auxesia reads the source tree once and builds a candidate.
Before publication, Auxesia computes the fingerprint again and compares it with the first fingerprint.
If the fingerprints are different, Auxesia retries once with a new read.
If the second attempt also changes, Auxesia rejects that candidate.
This comparison detects changes during the build but does not lock the source directory.

Auxesia publishes a complete valid snapshot in one atomic state change.
The snapshot includes the configuration, pages, static assets, discovery files, skill archives, and search index.
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

## Search index

Auxesia builds an in-memory Tantivy index before it publishes a snapshot.
If indexing fails at startup, the server does not start.
If indexing fails after startup, the previous pages and index remain available together.
If searchable content stays the same, the next snapshot reuses the index.
Asset-only edits do not rebuild it.

Indexing and search run on blocking workers, separate from async request processing.
Search has no disk cache, external service, or Node.js process.
The initial target is up to 1,000 pages.
An update can temporarily retain both the previous index and the candidate index in memory.

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

The server keeps file contents in memory.
It reads the metadata of the source tree on requests and reads file contents only after a change.
It targets modest documentation sites with local source files.
Large files and large trees increase memory use and refresh work.
Skill contents are instructions for the consumer.
Auxesia does not execute skill code.

## Deferred features

The current command serves a local directory.
It does not manage repository checkouts, Git synchronization, or webhooks.
Browser live updates and static export are also deferred.
The server currently has no authentication or authorization layer.

See [Configuration](/reference/configuration.md) for exclusions and [Skills](/reference/skills.md) for archive behavior.
