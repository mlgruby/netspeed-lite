# Build stage
FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install Ookla Speedtest CLI
RUN apt-get update && \
    apt-get install -y curl gnupg ca-certificates && \
    curl -s https://packagecloud.io/install/repositories/ookla/speedtest-cli/script.deb.sh | bash && \
    apt-get install -y speedtest && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/netspeed-lite /usr/local/bin/netspeed-lite

# Expose metrics port
EXPOSE 9109

# Set user to non-root
# Set user to non-root
# Set user to non-root
RUN useradd -m -s /bin/bash netspeed

# Ensure home directory exists and has correct permissions
RUN mkdir -p /home/netspeed/.config/ookla && \
    chown -R netspeed:netspeed /home/netspeed && \
    su netspeed -c "speedtest --accept-license --accept-gdpr"

ENV HOME=/home/netspeed
USER netspeed

# Run the binary
ENTRYPOINT ["/usr/local/bin/netspeed-lite"]
