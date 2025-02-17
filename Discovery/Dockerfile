# syntax=docker/dockerfile:1.6

FROM gitea.contc/controlplane/rust-builder-alpine:0.1.4 as source
WORKDIR /root/source/k8s-discovery
RUN echo changed
COPY --link . /root/source/k8s-discovery

FROM source as test
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/k8s-discovery/target \
    cargo test

FROM source as audit
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/k8s-discovery/target \
    cargo audit --color=always

FROM source as build-release
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/k8s-discovery/target \
    cargo build --release --bin k8s-discovery \
    && cp target/release/k8s-discovery .

FROM alpine:3.18 AS release
RUN apk add --no-cache openssl
COPY --from=build-release \
    /root/source/k8s-discovery/k8s-discovery \
    /usr/bin
CMD /usr/bin/k8s-discovery
