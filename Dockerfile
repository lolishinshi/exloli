FROM rust:latest as builder

WORKDIR /app
COPY . .
RUN cargo install --path .

FROM debian:buster
WORKDIR /app
RUN apt-get update && apt-get install -y sqlite3 libssl1.1 ca-certificates libxml2 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/exloli /usr/local/bin/exloli
CMD ["exloli"]
