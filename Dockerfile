# Dockerfile
FROM rust:latest AS builder

WORKDIR /app
COPY . .

RUN apt-get update && apt-get install -y pkg-config libssl-dev openssl \
    && cargo build --release

# Runtime container
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y openssh-client tar cron

WORKDIR /dockup
COPY --from=builder /app/target/release/dockup /usr/local/bin/dockup

# Optional: Create volume for persistent config
VOLUME ["/dockup/config"]

# Add entrypoint script
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]