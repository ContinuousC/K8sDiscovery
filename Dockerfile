# syntax=docker/dockerfile:1.6

FROM gitea.contc/controlplane/techdocs-builder:0.0.7 AS build-techdocs
WORKDIR /app
COPY --link mkdocs.yml /app/
COPY --link docs /app/docs
RUN npx @techdocs/cli generate --no-docker

FROM build-techdocs AS publish-techdocs
ENV NODE_TLS_REJECT_UNAUTHORIZED=0
ENV AWS_REGION=us-west-2
RUN --mount=type=secret,id=minio_credentials,target=/root/.minio.env,required=true \
    export $(cat /root/.minio.env); \
    npx @techdocs/cli publish \
        --publisher-type awsS3 \
	--storage-name techdocs \
        --entity default/component/k8s-discovery \
        --awsEndpoint https://minio.cortex \
        --awsS3ForcePathStyle

FROM gitea.contc/controlplane/rust-builder-alpine:0.1.4 as source
WORKDIR /root/source/k8s-discovery
COPY --link ./Discovery /root/source/k8s-discovery

FROM source AS build-rustdocs
RUN mkdir /root/docs
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/k8s-discovery/target \
    rm -rf /root/source/k8s-discovery/target/doc || true && \
    cargo doc --release --no-deps && \
    cp -a /root/source/k8s-discovery/target/doc/* /root/docs

FROM bitnami/minio-client AS publish-rustdocs
COPY --from=build-rustdocs /root/docs /docs
RUN --mount=type=secret,id=minio-config,target=/.mc/config.json,uid=1001,required=true \
    --mount=type=secret,id=contc-ca,target=/.mc/certs/CAs/contc.ca,uid=1001,required=true \
    mc mirror --overwrite --remove /docs minio/rustdocs/k8s-discovery
