FROM quay.io/influxdb/flux-build:latest

USER root

ENV NODE_VERSION="node_16.x"
ENV DEBIAN_NAME="bullseye"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource.gpg.key | apt-key add - && \
    echo "deb https://deb.nodesource.com/$NODE_VERSION $DEBIAN_NAME main" \
        > /etc/apt/sources.list.d/nodesource.list && \
    apt-get update && apt-get install --no-install-recommends -y nodejs wabt binaryen && \
    apt-get clean  && rm -rf /var/lib/apt/lists/*

# UNAME comes from the flux-build docker container
USER $UNAME
