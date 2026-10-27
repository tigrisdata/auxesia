---
name: serve-auxesia
description: Build Auxesia, serve its documentation, and inspect HTML, Markdown, discovery, and skill archive responses.
---

# Serve Auxesia

Use this skill in an Auxesia source checkout with Rust, Cargo, curl, and Python 3 available.
The commands use the repository root as the current directory.
Auxesia publishes these instructions. It does not run them.

## Build and run

1. Make sure that `Cargo.toml`, `crates/auxesia`, and `docs` exist in the current directory.
2. Build the executable:

   ```sh
   cargo build -p auxesia
   ```

3. Run the server in a terminal that can remain open:

   ```sh
   cargo run -p auxesia -- docs --listen 127.0.0.1:8080 --dev
   ```

4. Make sure that the terminal reports `Auxesia listening on http://127.0.0.1:8080`.

If port 8080 is occupied, select another port with `--listen`.
Use that port in each later request.
The `--dev` flag shows update errors on HTML pages.

## Inspect the responses

1. Open another terminal at the repository root.
2. Run the commands in [Response checks](references/checks.md).
3. Make sure that each response matches the expected result in that file.
4. If a response differs, read the server log before you change the content or code.
5. After the checks, press Ctrl-C in the server terminal.

Report the build result, the response results, and any server warnings.
Include the request URL for each failure.
