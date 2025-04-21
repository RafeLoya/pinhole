FROM docker.io/acfreeman/rustnetworking

WORKDIR /app

# Install additional dependencies
# Some others that might be useful:
#   build-essential coreutils valgrind \
#   libssl-dev pkg-config \
RUN apt-get update && apt-get install -y \
    lsof iputils-ping \
    && rm -rf /var/lib/apt/lists/* \
    && rustup default nightly

COPY server/ server/
COPY common/ common/

WORKDIR /app/server
RUN cargo build

# volumes are mounted in the yaml file
# VOLUME ["/app/server"]
# VOLUME ["/app/common"]

# Set environment variables
ENV RUST_LOG=info

# Expose port
EXPOSE 8080

# Start the server
# ENTRYPOINT ["./target/*/server"] # runs debug if both exist
