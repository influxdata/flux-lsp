#!/bin/bash

set -e

BUILD_MODE=${BUILD_MODE-release}

BUILD_FLAG=""
BUILD_MODE_ARGS=""
case $BUILD_MODE in
    "release")
        BUILD_FLAG="--release"
        ;;
    "dev")
        BUILD_FLAG="--dev"
        BUILD_MODE_ARGS="--features console_log"
        ;;
    *)
        echo "Invalid build mode: ${BUILD_MODE}"
        echo "Only 'release' and 'dev' build mode options are supported"
        exit 1
        ;;
esac

wasm-pack build \
    -t nodejs \
    -d pkg-node \
    --out-name flux-lsp-node \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS
wasm-pack build \
    -t browser \
    -d pkg-browser \
    --out-name flux-lsp-browser \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS

cat pkg-node/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-node\"/g > pkg-node/package-new.json
mv pkg-node/package-new.json pkg-node/package.json

cat pkg-browser/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-browser\"/g | sed -e 's/"files": \[/"files": [\
    "flux-lsp-browser_bg.js",/g' > pkg-browser/package-new.json
mv pkg-browser/package-new.json pkg-browser/package.json
