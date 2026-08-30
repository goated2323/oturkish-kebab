# Rust multi-stage pour O'Turkish Kebab
FROM rust:1.78-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/oturkish-kebab /app/oturkish-kebab
COPY index.html ./
COPY images ./images
ENV PORT=8080
ENV RUST_LOG=info
EXPOSE 8080
CMD ["./oturkish-kebab"]
