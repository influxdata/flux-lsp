#!/bin/bash

./build.sh

cd pkg-node
npm publish
cd ..

cd pkg-browser
npm publish
cd ..
