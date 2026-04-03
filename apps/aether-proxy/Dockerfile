FROM debian:bookworm-slim

ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY build/linux-${TARGETARCH}/aether-proxy /usr/local/bin/aether-proxy

ENTRYPOINT ["aether-proxy"]
