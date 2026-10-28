---
title: MCP reference
description: Connect an MCP client to /mcp to list skills and read their files with tools.
---

# MCP reference

Auxesia serves a Model Context Protocol (MCP) endpoint at `/mcp`.
An MCP client, such as Claude Code, can use this endpoint to find skills and read them.
The endpoint is always on and read-only.
Auxesia never runs a skill or a script from a skill bundle.

## Connect a client

1. Start Auxesia. See [Getting started](/getting-started.md).
2. Add the endpoint to your MCP client. For Claude Code, use this command:

   ```sh
   claude mcp add --transport http auxesia http://127.0.0.1:8080/mcp
   ```

3. Tell the agent to list the Auxesia skills.

The endpoint uses the Streamable HTTP transport.
It accepts the protocol versions `2024-11-05`, `2025-03-26`, `2025-06-18`, and `2025-11-25`.

## Tools

The server advertises tools only. It has no prompts and no resources.
All tools have the `readOnlyHint` annotation.

| Tool | Arguments | Result |
| --- | --- | --- |
| `list_skills` | None | A JSON array with the slug, name, description, page URL, and archive URL of each skill. |
| `get_skill` | `name` | The original `SKILL.md` text, then a JSON summary with the list of supporting files. |
| `read_skill_file` | `name`, `path` | The text of one file from the skill bundle. |

The `name` argument is the skill slug, such as `serve-auxesia`.
The `path` argument is relative to the skill folder, such as `references/checks.md`.
The `get_skill` result gives the exact `path` values.

`list_skills` does not show skills with `index: false`, the same as `/llms.txt`.
`get_skill` and `read_skill_file` accept these hidden skills by slug, the same as their direct URLs.

## Errors

A tool result with `isError: true` tells the agent how to correct the call.
These conditions cause this result:

- The skill slug does not exist.
- The file path is not in the skill bundle.
- A required argument is missing or is not a string.
- The file is not UTF-8 text. The message gives the URL of the `.skill` archive.

An unknown tool name causes the JSON-RPC error `-32602`.

## Live content

Each tool call refreshes the documentation snapshot, the same as an HTTP request.
An edit to a skill shows in the next tool call without a restart.
If an edit is not valid, the tools use the last valid snapshot.

## Security

The endpoint rejects a request with status `403` if its `Origin` header is not a loopback origin.
This check stops DNS-rebinding attacks from web pages.
Clients that do not send `Origin`, such as command-line agents, are accepted.

The source tree cannot define the `/mcp` and `/mcp/sse` routes because the MCP endpoint uses them.
For example, a top-level file named `mcp` or a top-level `mcp/` folder causes a build error.
