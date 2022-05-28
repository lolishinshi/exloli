FROM rust:latest as builder

WORKDIR /app
COPY . .
RUN cargo install --path .

FROM debian:bullseye
WORKDIR /app
RUN apt-get update && apt-get install -y sqlite3 libssl1.1 ca-certificates libxml2 && rm -rf /var/lib/apt/lists/*
RUN echo '/etc/ssl/openssl.cnf \
system_default = system_default_sect \
\
[system_default_sect] \
MinProtocol = TLSv1.2 \
CipherString = DEFAULT@SECLEVEL=1 \
' >> /etc/ssl/openssl.cnf
COPY --from=builder /usr/local/cargo/bin/exloli /usr/local/bin/exloli
CMD ["exloli"]
