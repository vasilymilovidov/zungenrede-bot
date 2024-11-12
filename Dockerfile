 FROM rust:1.75-slim as builder
   WORKDIR /app
   COPY . .
   RUN cargo build --release

   FROM debian:bullseye-slim
   WORKDIR /app
   COPY --from=builder /app/target/release/zungenrede-bot .
   COPY --from=builder /app/translations_storage.json .
   
   CMD ["./zungenrede-bot"]
