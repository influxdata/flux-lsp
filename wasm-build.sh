#!/bin/bash

source $HOME/.cargo/env

wasm-pack build -t nodejs -d pkg-node --out-name flux-lsp-node --scope influxdata --release
wasm-pack build -t browser -d pkg-browser --out-name flux-lsp-browser --scope influxdata --release

cat pkg-node/package.json | sed s/\\/flux-lsp\"/\\/flux-lsp-node\"/g > pkg-node/package-new.json
mv pkg-node/package-new.json pkg-node/package.json

cat pkg-browser/package.json | sed s/\\/flux-lsp\"/\\/flux-lsp-browser\"/g > pkg-browser/package-new.json
mv pkg-browser/package-new.json pkg-browser/package.json
