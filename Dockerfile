FROM rust:1.88-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY crates crates
COPY proto proto

RUN cargo build --release --bin bitwarden-csi-provider

FROM gcr.io/distroless/cc-debian12

LABEL org.opencontainers.image.source="https://github.com/kvncrw/bitwarden-csi-provider"
LABEL org.opencontainers.image.description="Bitwarden Secrets Manager provider for Kubernetes Secrets Store CSI Driver"
LABEL org.opencontainers.image.licenses="GPL-3.0-only"

COPY --from=builder /build/target/release/bitwarden-csi-provider /usr/local/bin/bitwarden-csi-provider

ENTRYPOINT ["bitwarden-csi-provider"]
