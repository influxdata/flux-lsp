#!/bin/bash

# This script generates the wasm build artifacts, optimizes them, and modifies
# the corresponding package data. If `BUILD_MODE=dev` is set, some handy
# tools and libraries are included in the binary for development purposes
# which would otherwise be inappropriate for the final build.
#
# Currently, two separate build artifacts are generated, one for nodejs and one
# for the browser. This is a by-product of using `wasm-pack`. Eventually, we
# should be able to use a `--target=all` to generate a single package that is
# platform agnostic.
# For more information, see https://github.com/rustwasm/wasm-pack/issues/313

set -e

TARGET_DIR=target
NODE_DIR=$TARGET_DIR/pkg-node
BROWSER_DIR=$TARGET_DIR/pkg-browser

BUILD_MODE=${BUILD_MODE-release}

BUILD_FLAG=""
BUILD_MODE_ARGS="--no-default-features --features wasm"
case $BUILD_MODE in
    "release")
        BUILD_FLAG="--release"
        ;;
    "dev")
        BUILD_FLAG="--dev"
        BUILD_MODE_ARGS="${BUILD_MODE_ARGS},console_log,console_error_panic_hook"
        ;;
    *)
        echo "Invalid build mode: ${BUILD_MODE}"
        echo "Only 'release' and 'dev' build mode options are supported"
        exit 1
        ;;
esac

wasm-pack build \
    -t nodejs \
    -d $NODE_DIR \
    --out-name flux-node \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS
wasm-pack build \
    -t browser \
    -d $BROWSER_DIR \
    --out-name flux-browser \
    --scope influxdata \
    ${BUILD_FLAG} \
    -- \
    --locked \
    $BUILD_MODE_ARGS

# Strip producers header and some other optional bits.
wasm-strip $NODE_DIR/flux-node_bg.wasm
wasm-opt -Oz -o $NODE_DIR/flux-node_bg.wasm $NODE_DIR/flux-node_bg.wasm
wasm-strip $BROWSER_DIR/flux-browser_bg.wasm
wasm-opt -Oz -o $BROWSER_DIR/flux-browser_bg.wasm $BROWSER_DIR/flux-browser_bg.wasm

cat $NODE_DIR/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-node\"/g > $NODE_DIR/package-new.json
mv $NODE_DIR/package-new.json $NODE_DIR/package.json
echo "" > $NODE_DIR/README.md

cat $BROWSER_DIR/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-browser\"/g > $BROWSER_DIR/package-new.json
mv $BROWSER_DIR/package-new.json $BROWSER_DIR/package.json
echo "" > $BROWSER_DIR/README.md
