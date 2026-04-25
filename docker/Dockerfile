FROM rust:1.87-slim AS builder

WORKDIR /app
COPY . .

RUN cargo build --release -p lexrs-server --bin writer --bin reader

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/writer /usr/local/bin/writer
COPY --from=builder /app/target/release/reader /usr/local/bin/reader
