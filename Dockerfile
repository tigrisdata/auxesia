# syntax=docker/dockerfile:1
FROM rust:1.99.1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
ARG TARGETARCH
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target,id=auxesia-target-${TARGETARCH} \
    cargo build --locked --release --package auxesia \
    && install -D target/release/auxesia /out/auxesia

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
