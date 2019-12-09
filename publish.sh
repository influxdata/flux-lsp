#!/bin/bash

./build.sh

cd pkg-node
npm publish --access public
cd ..

cd pkg-browser
npm publish --access public
cd ..
