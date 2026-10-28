# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.99.1-bookworm AS build
ARG CARGO_ZIGBUILD_VERSION=0.24.0
ARG ZIG_VERSION=0.14.1
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3-venv \
    && rm -rf /var/lib/apt/lists/* \
    && python3 -m venv /opt/zigbuild \
    && /opt/zigbuild/bin/pip install --no-cache-dir \
        "cargo-zigbuild==${CARGO_ZIGBUILD_VERSION}" "ziglang==${ZIG_VERSION}"
ENV PATH="/opt/zigbuild/bin:${PATH}"
ARG TARGETARCH
RUN case "$TARGETARCH" in \
        amd64) target=x86_64-unknown-linux-gnu ;; \
        arm64) target=aarch64-unknown-linux-gnu ;; \
        *) echo "Unsupported target architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$target" \
    && printf '%s' "$target" > /rust-target
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target,id=auxesia-zigbuild-target-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/root/.cache/zig,id=auxesia-zig-${TARGETARCH},sharing=locked \
    target="$(cat /rust-target)" \
    && cargo zigbuild --locked --release --package auxesia --target "${target}.2.36" \
    && install -D "target/${target}/release/auxesia" /out/auxesia

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="Auxesia" \
      org.opencontainers.image.description="Markdown documentation and downloadable skills" \
      org.opencontainers.image.source="https://github.com/tigrisdata/auxesia" \
      org.opencontainers.image.licenses="MIT"
COPY --from=build /out/auxesia /usr/local/bin/auxesia
COPY --chown=65532:65532 docs/ /docs/
USER 65532:65532
WORKDIR /docs
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/auxesia"]
CMD ["/docs", "--listen", "0.0.0.0:8080"]
