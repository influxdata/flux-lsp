FROM quay.io/influxdb/flux-build:latest

USER root

ENV NODE_VERSION="node_12.x"
ENV DEBIAN_NAME="buster"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource.gpg.key | apt-key add - && \
    echo "deb https://deb.nodesource.com/$NODE_VERSION $DEBIAN_NAME main" \
        > /etc/apt/sources.list.d/nodesource.list && \
    apt-get update && apt-get install --no-install-recommends -y nodejs && \
    apt-get clean  && rm -rf /var/lib/apt/lists/*

USER $USER
