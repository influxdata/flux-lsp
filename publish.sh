#!/bin/bash
set -o xtrace

CWD=`pwd`

cd $CWD/target/pkg-node
npm publish --access public

cd $CWD/target/pkg-browser
npm publish --access public
