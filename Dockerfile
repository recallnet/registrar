FROM rust:latest AS builder
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

RUN cargo build --release


######################

FROM scratch
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
COPY --from=builder /etc/ssl/certs /etc/ssl/certs
COPY --from=builder /lib /lib
COPY --from=builder /usr/lib /usr/lib
COPY --from=builder /usr/src/faucet/target/release/faucet /bin/app

WORKDIR /bin
USER faucet:faucet
EXPOSE 8080

ENTRYPOINT ["/bin/app"]
