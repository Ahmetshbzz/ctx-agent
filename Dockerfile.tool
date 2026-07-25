FROM rust:1.97.1-bookworm AS build
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends cmake libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release \
    && strip target/release/ctx \
    && test -x target/release/ctx

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 zlib1g \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/ctx /usr/local/bin/ctx-agent
ENTRYPOINT ["/usr/local/bin/ctx-agent"]
