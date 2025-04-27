FROM docker.io/acfreeman/rustnetworking

WORKDIR /app

# Install additional dependencies
RUN apt-get update && apt-get install -y \
    lsof iputils-ping \
    && rm -rf /var/lib/apt/lists/* \
    && rustup default nightly

# Copy source code
COPY server/ server/
COPY common/ common/

# Build the server
WORKDIR /app/server
RUN cargo build --release

RUN cp target/release/server /app/server_binary

# Set environment variables
ENV RUST_LOG=info

# Expose port
EXPOSE 4433/udp

# Set the working directory and entrypoint
WORKDIR /app
ENTRYPOINT ["./server_binary"]
