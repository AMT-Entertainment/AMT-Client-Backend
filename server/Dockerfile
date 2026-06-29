FROM rust:bookworm AS builder
RUN apt-get update && apt-get install -y gcc pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/amt-social-server /usr/local/bin/
EXPOSE 8080
ENV DB_PATH=/data/amt_social.db
VOLUME ["/data"]
CMD ["amt-social-server"]
