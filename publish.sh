#!/bin/bash

./build.sh

cd pkg-node
npm publish --dry-run
cd ..

cd pkg-browser
npm publish --dry-run
cd ..
