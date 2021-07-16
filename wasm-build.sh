#!/bin/bash

EXTRA_ARGS="--"
if [[ ! -z "${LSP2}" ]]; then
    echo "Building with --featurs lsp2"
    EXTRA_ARGS="${EXTRA_ARGS} --features lsp2"
fi

wasm-pack build -t nodejs -d pkg-node --out-name flux-lsp-node --scope influxdata --release $EXTRA_ARGS
wasm-pack build -t browser -d pkg-browser --out-name flux-lsp-browser --scope influxdata --release $EXTRA_ARGS

cat pkg-node/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-node\"/g > pkg-node/package-new.json
mv pkg-node/package-new.json pkg-node/package.json

cat pkg-browser/package.json | sed s/@influxdata\\/flux-lsp\"/@influxdata\\/flux-lsp-browser\"/g | sed -e 's/"files": \[/"files": [\
    "flux-lsp-browser_bg.js",/g' > pkg-browser/package-new.json
mv pkg-browser/package-new.json pkg-browser/package.json
