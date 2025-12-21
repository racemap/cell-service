FROM rust:1.92.0-trixie AS builder
WORKDIR /usr/src/racemap-cell-service
COPY . .
RUN cargo install diesel_cli --no-default-features --features mysql
RUN cargo install --path .

FROM rust:1.92.0-slim-trixie

RUN apt-get update && apt-get install -y default-libmysqlclient-dev libssl-dev && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/racemap-cell-service /usr/local/bin/racemap-cell-service
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel
COPY ./migrations ./migrations
COPY ./run.sh ./run.sh
RUN chmod +x /run.sh

ENTRYPOINT ["/run.sh"]
CMD ["racemap-cell-service"]