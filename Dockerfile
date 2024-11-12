FROM rust:1.75-slim as builder

# Install required dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

# Create empty translations file if it doesn't exist
RUN touch translations_storage.json

FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y libssl1.1 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/zungenrede-bot .
COPY --from=builder /app/translations_storage.json .

# Ensure the file is writable
RUN touch translations_storage.json && \
    chmod 666 translations_storage.json

CMD ["./zungenrede-bot"]
