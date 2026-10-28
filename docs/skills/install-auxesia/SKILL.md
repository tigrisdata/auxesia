---
navbar: Install Auxesia
name: install-auxesia
description: Use when a user wants to install or deploy Auxesia to serve documentation through Docker, Kubernetes, or an HTTP reverse proxy.
---

# Install Auxesia

Guide the user from their chosen environment to a working documentation URL.
Use the published image at `ghcr.io/tigrisdata/auxesia`.
Auxesia serves these instructions. The agent or user runs the installation commands.

## Ask where and how to install

Start with the questions that the user has not already answered:

- Where will Auxesia run: local Docker, Docker Compose on a server, Kubernetes, or another container platform?
- Which host or cluster and namespace will contain the installation?
- Where are the docs: an existing folder, a Git repository, or the bundled Auxesia documentation?
- For Git, which repository, revision, and documentation subdirectory will you use? Does the repository need credentials?
- Which hostname will serve the docs? Is access public, private, or local only?
- Which HTTP router handles that hostname: Nginx, Caddy, Traefik, an Ingress controller, or a Gateway?
- How does that environment manage DNS, TLS certificates, and authentication?

Wait for the missing choices before you select a deployment method.
Use the existing deployment conventions where available.
Then summarize the target, image, documentation path, and route.
Walk through the matching steps with concrete commands and configuration for those choices.
If you cannot access the target, give the user each command and ask for its result.

## Pull the image

For Docker or Compose, pull the selected image:

```sh
docker pull ghcr.io/tigrisdata/auxesia:latest
```

Use an available version tag or digest for a fixed release.
For Kubernetes, set the workload image to the same registry reference.
Set `imagePullPolicy: Always` for `latest` so new Pods pull the current image.
If the pull fails, resolve the registry error before you continue.

The image includes documentation at `/docs` and runs as `65532:65532`.
Its default command serves `/docs` on `0.0.0.0:8080`.
No Rust installation or Auxesia source build is required.

## Put the documentation inside the container

Choose one source:

- **Existing folder:** Mount the absolute host path at `/docs`, read-only.
- **Git repository:** Clone the selected revision before Auxesia starts. Share the checkout with the Auxesia container, read-only.
- **Bundled documentation:** Keep the image's `/docs` folder without a replacement mount.

For a host checkout, clone into a dedicated directory without overwriting existing files.
Mount only the documentation subdirectory at `/docs`.
For private repositories, use the environment's secret mechanism for Git credentials.
Keep credentials outside the served directory.

Make sure that user `65532:65532` can read the files and traverse their directories.
Documentation pages require YAML frontmatter with `title`. Skill entrypoints require `name`.
The optional `.auxesia.yaml` belongs at the documentation root.

### Docker or Compose

Adapt this example to the selected host folder and port:

```sh
docker run -d --name auxesia-docs --restart unless-stopped --read-only \
  -p 127.0.0.1:8080:8080 \
  --mount type=bind,src=/absolute/path/to/docs,dst=/docs,readonly \
  ghcr.io/tigrisdata/auxesia:latest
```

Replace `/absolute/path/to/docs` before execution.
For Compose, use the same image, mount, listen port, and restart policy.
If the router runs on the host, use `127.0.0.1:8080` as its upstream.
If the router runs in a container, join both services to a shared container network.
Use the Auxesia service name and port `8080` as that router's upstream.
Container loopback addresses refer to the router itself.

### Kubernetes with a Git checkout

Read the [objgit Kustomize example](https://github.com/tigrisdata/objgit/blob/main/manifest/docs/kustomization.yaml) and its referenced Deployment, Service, and Ingress.
Adapt this structure to the user's cluster:

1. Create an `emptyDir` volume for the checkout.
2. Add an init container that clones the selected repository and revision into that volume.
3. Mount the volume at `/git` in both containers.
4. Mount it read-only in the Auxesia container.
5. Set Auxesia arguments to the documentation path, `--listen`, and `0.0.0.0:8080`.
6. Set a readiness probe to `GET /llms.txt` on port `8080`.
7. Create a ClusterIP Service with matching Pod selectors and `port: 80`, `targetPort: 8080`.
8. Include the Deployment, Service, and route resources in `kustomization.yaml`.

For example, a checkout at `/git/project` with docs in `docs/` uses these arguments:

```yaml
args: ["/git/project/docs", "--listen", "0.0.0.0:8080"]
```

The objgit example uses `git-sync` with `--one-time` as an init container.
It shares the checkout through an `emptyDir` and sets `fsGroup: 65532` for volume access.
Auxesia resolves its root path once at startup and rejects symbolic links inside the documentation tree.
A continuously swapped git-sync root link does not update that resolved path.
Use a new Pod to load a new checkout.
Projected ConfigMap files use symbolic links, so do not mount them directly as documentation.

Adapt resource names, namespace, Git source, and security settings to the target.
The example's `runtimeClassName: kata`, `nginx` Ingress class, and `letsencrypt-prod` issuer are specific to its cluster.
Use those settings only if the target supports them.

## Register the HTTP route

Configure the chosen router to forward the selected hostname to Auxesia:

- **Host reverse proxy:** Forward to the published loopback port.
- **Container reverse proxy:** Forward to the Auxesia service name on port `8080` through the shared network.
- **Kubernetes Ingress:** Route the hostname with path `/` to the Service on port `80`.
- **Gateway API:** Create an HTTPRoute with the existing Gateway as its parent and the Service as its backend.

Use a dedicated hostname with Auxesia at `/`.
Auxesia generates root-relative links, so a stripped path prefix alone does not provide subpath support.
Preserve request paths and the `Accept` header for Markdown responses.
Forward `/llms.txt`, `/_auxesia/`, skill downloads, and `/mcp` along with page requests.
For MCP clients, allow streaming responses at `/mcp`.

Point DNS at the router and configure TLS through the environment's certificate mechanism.
Auxesia itself provides HTTP without TLS or authentication.
For private docs, apply the chosen access controls at the router.
For local-only use, the loopback port is the access URL. No public DNS or TLS configuration is required.

Render the manifests or validate the router configuration before deployment.
Apply them through the environment's normal deployment process.
For Kustomize, use `kubectl kustomize <manifest-directory>` before `kubectl apply -k <manifest-directory>`.
Use the selected cluster context and namespace explicitly.

## Verify the installation

1. Read the container logs and confirm that Auxesia starts without content errors.
2. Request `/llms.txt` through the internal upstream or a Kubernetes port-forward.
3. Request the final URL through the HTTP router.
4. Confirm that a known documentation page returns HTML.
5. Request that page with `Accept: text/markdown` and confirm that it returns the source Markdown.
6. Confirm that styles, assets, and discovery links work through the final hostname.
7. If the docs contain skills, confirm that a skill page and its `.skill` archive work.

Use [Response checks](references/checks.md) for the bundled documentation's detailed response checks.
Select actual page and skill URLs from the installed site's `/llms.txt` before you run them.
Adapt archive entry names to the installed skill name.

If a request fails, inspect the matching layer: clone, mount, container, Service, router, DNS, or TLS.
Keep the installation running after verification.
Report the final URL, image reference, documentation source, container path, and verification results.
Explain how to publish documentation updates in the chosen environment.
For an init-container checkout, restart the Deployment to clone again.
For a pinned Git revision, update that revision before the rollout.
