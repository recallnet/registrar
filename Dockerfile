FROM rust:latest
WORKDIR /usr/src/faucet
COPY . .
RUN rustup component add rustfmt clippy
RUN cargo build --release
EXPOSE 8080

ENTRYPOINT ["sh", "-c", "exec ./target/release/faucet --private-key $PRIVATE_KEY --token-address $TOKEN_ADDRESS"]