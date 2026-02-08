FROM rust:1.88-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd -m -u 10001 indexer
WORKDIR /app
COPY --from=builder /app/target/release/bitcoin-blockchain-indexer /usr/local/bin/bitcoin-blockchain-indexer
USER indexer
EXPOSE 8080
CMD ["/usr/local/bin/bitcoin-blockchain-indexer"]
