FROM rust:1.75-slim as builder

# Install required dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

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
