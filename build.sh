#!/bin/bash

source ./vars.sh

docker run \
    --rm \
    -v "$DIR:$SRC_DIR" \
    --env AR=llvm-ar \
    $imagename:$imagetag ./wasm-build.sh
