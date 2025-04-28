# Build stage
FROM rust:slim-bookworm AS builder

WORKDIR /app

COPY server/ server/
COPY common/ common/
COPY cicd/server.Cargo.toml Cargo.toml

RUN cargo build --release --bin server

# Runtime stage
# FROM debian:bookworm-slim
FROM docker.io/acfreeman/rustnetworking

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r appuser && useradd -r -g appuser -m appuser

WORKDIR /app

COPY --from=builder /app/target/release/server /app/server

RUN mkdir -p /app/logs && \
    chown -R appuser:appuser /app && \
    chmod 731 /app/logs

EXPOSE 8080/tcp
EXPOSE 443/udp

USER appuser

CMD ["/app/server"]
