---
name: serve-auxesia
description: Run Auxesia from its Docker image and inspect documentation, Markdown, discovery, and skill archive responses.
---

# Serve Auxesia

Use this skill to serve documentation with `ghcr.io/tigrisdata/auxesia`.
Docker, curl, and Python 3 must be available. A source checkout and Rust are not required.
Auxesia publishes these instructions. It does not run them.

## Run the bundled documentation

1. Pull the image:

   ```sh
   docker pull ghcr.io/tigrisdata/auxesia:latest
   ```

2. Run the server in a terminal that can remain open:

   ```sh
   docker run --rm --name auxesia-docs --read-only \
     -p 127.0.0.1:8080:8080 \
     ghcr.io/tigrisdata/auxesia:latest
   ```

3. Make sure that the terminal reports `Auxesia listening on http://0.0.0.0:8080`.

The image includes the Auxesia documentation at `/docs` and runs as user `65532:65532`.
The port mapping makes it available at `http://127.0.0.1:8080` on the host.
If host port 8080 is occupied, use a mapping such as `127.0.0.1:9000:8080` and adjust later requests.
Use a published version tag or digest instead of `latest` when a fixed image version is required.
If the image pull fails, report the error before attempting another deployment method.

## Serve another documentation folder

To serve an existing `docs` folder, stop the first container and run:

```sh
docker run --rm --name auxesia-docs --read-only \
  -p 127.0.0.1:8080:8080 \
  --mount type=bind,src="$PWD/docs",dst=/docs,readonly \
  ghcr.io/tigrisdata/auxesia:latest
```

Replace `$PWD/docs` with the absolute path to the desired folder.
The container user needs permission to read its files and access its directories.
The mount replaces the bundled documentation.
For developer-mode error toasts, append `/docs --listen 0.0.0.0:8080 --dev` after the image name.

## Inspect the responses

1. Open another terminal.
2. Run the commands in [Response checks](references/checks.md).
3. Make sure that each response matches the expected result in that file.
4. If a response differs, read the server log before you change the content or code.
5. After the checks, press Ctrl-C in the server terminal.

The response checks target the bundled documentation. For a mounted folder, use its page and skill URLs instead.

Report the image tag or digest, the response results, and any server warnings.
Include the request URL for each failure.
