#!/bin/bash

set -e

TARGET_DIR="target/"
TARGET_NODE_DIR="${TARGET_DIR}pkg-node"
TARGET_BROWSER_DIR="${TARGET_DIR}pkg-browser"

BUILD_MODE=${BUILD_MODE-release}

BUILD_FLAG=""
BUILD_MODE_ARGS="--no-default-features --features wasm,console_error_panic_hook"
case $BUILD_MODE in
    "release")
        BUILD_FLAG="--release"
        ;;
    "dev")
        BUILD_FLAG="--dev"
        BUILD_MODE_ARGS="${BUILD_MODE_ARGS},console_log"
        ;;
    *)
        echo "Invalid build mode: ${BUILD_MODE}"
        echo "Only 'release' and 'dev' build mode options are supported"
        exit 1
        ;;
esac

wasm-pack build \
    -t nodejs \
    -d $TARGET_NODE_DIR \
    --out-name flux-lsp-node \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS
wasm-pack build \
    -t bundler \
    -d $TARGET_BROWSER_DIR \
    --out-name flux-lsp-browser \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS

# Strip producers header and some other optional bits.
wasm-strip $TARGET_NODE_DIR/flux-lsp-node_bg.wasm
wasm-opt -Oz -o $TARGET_NODE_DIR/flux-lsp-node_bg.wasm $TARGET_NODE_DIR/flux-lsp-node_bg.wasm
wasm-strip $TARGET_BROWSER_DIR/flux-lsp-browser_bg.wasm
wasm-opt -Oz -o $TARGET_BROWSER_DIR/flux-lsp-browser_bg.wasm $TARGET_BROWSER_DIR/flux-lsp-browser_bg.wasm

cat $TARGET_NODE_DIR/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-node\"/g > $TARGET_NODE_DIR/package-new.json
mv $TARGET_NODE_DIR/package-new.json $TARGET_NODE_DIR/package.json

cat $TARGET_BROWSER_DIR/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-browser\"/g | sed -e 's/"files": \[/"files": [\
    "flux-lsp-browser_bg.js",/g' > $TARGET_BROWSER_DIR/package-new.json
mv $TARGET_BROWSER_DIR/package-new.json $TARGET_BROWSER_DIR/package.json
