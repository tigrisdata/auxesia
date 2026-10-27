---
title: Deploy with Docker
description: Build the Docker image, serve a documentation folder, and publish images to GHCR.
---
# Deploy with Docker

The Docker image includes Auxesia and its documentation at `/docs`. The server listens on `0.0.0.0:8080` inside the container.

The image builds Auxesia with Rust 1.99.1 on Debian Bookworm. The runtime uses Debian Bookworm Slim and runs as user `65532:65532`.

## Build and run locally

From the repository root, build the image:

```sh
docker build -t auxesia:local .
```

Run the bundled documentation:

```sh
docker run --rm -p 127.0.0.1:8080:8080 auxesia:local
```

Open <http://127.0.0.1:8080/> in a browser.

To request the original Markdown, run:

```sh
curl -H 'Accept: text/markdown' http://127.0.0.1:8080/
```

## Mount a documentation folder

From the repository root, mount your `docs` folder:

```sh
docker run --rm \
  -p 127.0.0.1:8080:8080 \
  --mount type=bind,src="$PWD/docs",dst=/docs,readonly \
  auxesia:local
```

The mount replaces the bundled documentation. The container user needs read access to the files and search access to their directories.

For another folder, replace `$PWD/docs` with its absolute path. Auxesia reads the mounted folder on each `GET` or `HEAD` request.

The image supports a read-only root filesystem. To use it, add `--read-only` to the command.

## Run with Compose

From the repository root, run:

```sh
docker compose up --build
```

The Compose service mounts `./docs` at `/docs` as read-only. It publishes port 8080 on `127.0.0.1`.

The service also uses a read-only root filesystem, drops Linux capabilities, and prevents new privileges.

To stop the service, run:

```sh
docker compose down
```

## Publish to GHCR

The repository workflow configures publication to `ghcr.io/tigrisdata/auxesia`. Publication occurs after a push to `main` or a `v*` tag in `tigrisdata/auxesia`.

The workflow first requires Rust format checks, Clippy, tests, and a Docker smoke test to pass. Pull requests run these checks without publication.

The workflow publishes images for `linux/amd64` and `linux/arm64` with these tags:

| Push | Image tags |
| --- | --- |
| `main` | `main`, `latest`, and `sha-<full-commit-SHA>` |
| `v1.2.3` | `v1.2.3`, `1.2.3`, and `sha-<full-commit-SHA>` |
| `v1.2.3-rc.1` | `v1.2.3-rc.1`, `1.2.3-rc.1`, and `sha-<full-commit-SHA>` |

Only a `main` push updates `latest`. Version tags, including prerelease tags, do not update `latest`.

After the workflow publishes an image, run it:

```sh
docker run --rm -p 127.0.0.1:8080:8080 \
  ghcr.io/tigrisdata/auxesia:latest
```

The workflow uses `GITHUB_TOKEN` with package write permission to publish the image.

## Public access

Auxesia serves HTTP without TLS or authentication. For public access, place a reverse proxy in front of the server.

The reverse proxy supplies HTTPS and any required authentication. Configure its upstream to reach port 8080 on the container.

The [operations reference](/reference/operations.md) describes server updates and startup errors.
