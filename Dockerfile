FROM rust:latest AS builder
RUN apt-get update && apt-get install -y musl-tools curl perl make gcc
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/src/faucet
COPY ./ .
RUN rustup component add rustfmt clippy
ENV USER=faucet
ENV UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

# Build OpenSSL statically
RUN curl -L https://github.com/openssl/openssl/releases/download/openssl-3.3.1/openssl-3.3.1.tar.gz -o openssl-3.3.1.tar.gz && \
    tar -xzf openssl-3.3.1.tar.gz && \
    cd openssl-3.3.1 && \
    ./Configure no-shared no-async linux-x86_64 -fPIC --prefix=/usr/local/ssl && \
    make -j$(nproc) && \
    make install_sw install_ssldirs && \
    cd .. && \
    rm -rf openssl-3.3.1 openssl-3.3.1.tar.gz

# Set environment variables for static linking
ENV OPENSSL_DIR=/usr/local/ssl
ENV OPENSSL_STATIC=1
ENV PKG_CONFIG_PATH=/usr/local/ssl/lib64/pkgconfig
ENV LD_LIBRARY_PATH=/usr/local/ssl/lib64:$LD_LIBRARY_PATH

# Build the Rust application
RUN RUSTFLAGS='-C target-feature=+crt-static' \
    CC=musl-gcc \
    cargo build --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
COPY --from=builder /usr/src/faucet/target/x86_64-unknown-linux-musl/release/faucet /bin/app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /usr/local/ssl/lib64 /usr/local/ssl/lib64
USER faucet:faucet
EXPOSE 8080
CMD ["/bin/app"]
