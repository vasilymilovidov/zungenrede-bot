# Use Debian as base image
FROM debian:bookworm-slim as builder

# Install Rust and required dependencies
RUN apt-get update && \
    apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust using rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/zungenrede-bot .

# Create directory for storage
RUN mkdir -p /app/data && \
    chmod 777 /app/data

ENV STORAGE_FILE=/app/data/translations_storage.json
ENV ALLOWED_USERS=""

CMD ["./zungenrede-bot"]
