FROM rust:1.93-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release --bin server

FROM debian:bookworm-slim

COPY --from=builder /app/target/release/server /usr/local/bin/server

ENV PORT=3001
EXPOSE 3001

CMD ["server"]
