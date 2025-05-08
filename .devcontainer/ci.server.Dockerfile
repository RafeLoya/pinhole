# Using this version for server debugging in VM
FROM docker.io/acfreeman/rustnetworking

WORKDIR /app

RUN apt-get update && apt-get install -y \
    lsof iputils-ping tshark \
    && rm -rf /var/lib/apt/lists/* \
    && rustup default nightly

COPY . .

EXPOSE 8080/tcp
EXPOSE 443/udp
EXPOSE 4433/udp

CMD ["/bin/sh", "-c", "cargo run --release --bin server && while sleep 1000; do :; done"]
