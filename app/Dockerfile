# syntax=docker/dockerfile:1
# API server image. Build from the repository root:
#   docker build -t githubissuemanager/server:latest app/

FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY middleware middleware
COPY server server
# Migrations and the GraphQL schema are embedded at compile time.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p server \
    && cp target/release/server /usr/local/bin/server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 app \
    && useradd --uid 10001 --gid app --no-create-home --shell /usr/sbin/nologin app
COPY --from=build /usr/local/bin/server /usr/local/bin/server
USER 10001:10001
ENV BIND_ADDRESS=0.0.0.0:8080 \
    LOG_FORMAT=json \
    RUST_LOG=info,sqlx=warn
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 CMD ["server", "--healthcheck"]
ENTRYPOINT ["server"]
