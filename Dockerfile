# Stage 1: Build
FROM rust:1-bookworm AS builder

WORKDIR /build

# Install dependencies for building
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/oxidized-nbt/Cargo.toml crates/oxidized-nbt/Cargo.toml
COPY crates/oxidized-macros/Cargo.toml crates/oxidized-macros/Cargo.toml
COPY crates/oxidized-types/Cargo.toml crates/oxidized-types/Cargo.toml
COPY crates/oxidized-protocol/Cargo.toml crates/oxidized-protocol/Cargo.toml
COPY crates/oxidized-world/Cargo.toml crates/oxidized-world/Cargo.toml
COPY crates/oxidized-game/Cargo.toml crates/oxidized-game/Cargo.toml
COPY crates/oxidized-server/Cargo.toml crates/oxidized-server/Cargo.toml

# Create stub source files so cargo can resolve the dependency graph
RUN mkdir -p crates/oxidized-nbt/src && echo "" > crates/oxidized-nbt/src/lib.rs && \
    mkdir -p crates/oxidized-macros/src && echo "" > crates/oxidized-macros/src/lib.rs && \
    mkdir -p crates/oxidized-types/src && echo "" > crates/oxidized-types/src/lib.rs && \
    mkdir -p crates/oxidized-protocol/src && echo "" > crates/oxidized-protocol/src/lib.rs && \
    mkdir -p crates/oxidized-world/src && echo "" > crates/oxidized-world/src/lib.rs && \
    mkdir -p crates/oxidized-game/src && echo "" > crates/oxidized-game/src/lib.rs && \
    mkdir -p crates/oxidized-server/src && echo "fn main() {}" > crates/oxidized-server/src/main.rs

# Pre-build dependencies (cached unless Cargo.toml/Cargo.lock change)
RUN cargo build --release -p oxidized-server 2>/dev/null || true

# Copy real source code
COPY crates/ crates/

# Touch source files to invalidate the stub builds
RUN find crates -name "*.rs" -exec touch {} +

# Build the actual binary
RUN cargo build --release -p oxidized-server && \
    cp target/release/oxidized /oxidized

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash oxidized

COPY --from=builder /oxidized /usr/local/bin/oxidized

# Server data directory
WORKDIR /data
RUN chown oxidized:oxidized /data

USER oxidized

# Minecraft server port
EXPOSE 25565/tcp

# Volume for persistent world data
VOLUME ["/data"]

ENTRYPOINT ["oxidized"]
