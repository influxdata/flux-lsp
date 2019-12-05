#!/bin/bash

imagename=flux-lsp-builder

SRC_DIR=/src
DIR=$(pwd)

# Build new docker image
docker build \
    -f Dockerfile \
    -t $imagename \
    $DIR

docker run \
    --rm \
    --name $imagename \
    -v "$DIR:$SRC_DIR" \
    --env AR=llvm-ar \
    $imagename ./wasm-build.sh
