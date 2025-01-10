FROM --platform=$BUILDPLATFORM ubuntu:jammy AS builder
USER root

RUN apt-get update && \
    apt-get install -y build-essential clang cmake protobuf-compiler curl \
    openssl libssl-dev pkg-config git-core ca-certificates && \
    update-ca-certificates

RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y
ENV PATH="/root/.cargo/bin:${PATH}"

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++

ARG RUST_VERSION=1.82.0
RUN \
    rustup install ${RUST_VERSION} && \
    rustup default ${RUST_VERSION} && \
    rustup target add aarch64-unknown-linux-gnu

# Defined here so anything above it can be cached as a common dependency.
ARG TARGETARCH

RUN if [ "${TARGETARCH}" = "arm64" ]; then \
    apt-get install -y g++-aarch64-linux-gnu libc6-dev-arm64-cross; \
    rustup target add aarch64-unknown-linux-gnu; \
    rustup toolchain install ${RUST_VERSION}-aarch64-unknown-linux-gnu; \
    fi

WORKDIR /usr/src/registrar
COPY ./Cargo.* ./
COPY ./src/ ./src/
# RUN rustup component add rustfmt clippy
ENV USER=registrar
ENV UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

RUN \
    --mount=type=cache,target=/root/.cargo/registry,sharing=locked \
    --mount=type=cache,target=/root/.cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    set -eux; \
    case "${TARGETARCH}" in \
    amd64) ARCH='x86_64'  ;; \
    arm64) ARCH='aarch64' ;; \
    esac; \
    cargo build --release --locked --target ${ARCH}-unknown-linux-gnu; \
    mv ./target/${ARCH}-unknown-linux-gnu/release/registrar ./


FROM --platform=$BUILDPLATFORM ubuntu:jammy
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
COPY --from=builder /usr/src/registrar/registrar /bin/app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
USER registrar:registrar
EXPOSE 8080
CMD ["/bin/app", "start"]
