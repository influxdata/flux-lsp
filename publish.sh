#!/bin/bash

cd target/pkg-node
npm publish --access public
cd ..

cd target/pkg-browser
npm publish --access public
