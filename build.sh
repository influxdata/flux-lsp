#!/bin/bash

source ./vars.sh

if [[ "$(docker images -q $imagename:$imagetag 2> /dev/null)" == "" ]]; then
  echo "Building docker image"
  ./build-docker-image.sh
fi

docker run \
    --rm \
    --name $imagename \
    -v "$DIR:$SRC_DIR" \
    --env AR=llvm-ar \
    $imagename ./wasm-build.sh
