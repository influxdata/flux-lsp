#!/bin/sh
# flux-lsp has a number of direct downstream consumers maintained adjacent to flux-lsp, and which drive
# features and changes in flux-lsp. This script is designed to be run after the release machinery completes
# and released version's build artifacts are available via npm.
set -e

if [[ ! $(command -v hub) ]]; then
    echo "Please install the hub tool and re-run."
    exit 1
fi
if [[ ! $(command -v npm) ]]; then
    echo "Please install the npm and re-run."
    exit 1
fi
if [[ ! $(command -v yarn) ]]; then
    echo "Please install the yarn tool and re-run."
    exit 1
fi

VERSION=`curl -s https://api.github.com/repos/influxdata/flux-lsp/releases/latest | jq -r .tag_name`

TEMPDIR=$(mktemp -d -t flux-lsp-release.XXXX)
echo "Using fresh install in $TEMPDIR"
cd $TEMPDIR

git clone git@github.com:influxdata/ui.git > /dev/null 2>&1
git clone git@github.com:influxdata/vsflux.git > /dev/null 2>&1

cd $TEMPDIR/ui
git checkout -b build/update-lsp-to-${VERSION}
yarn upgrade @influxdata/flux-lsp-browser@${VERSION}
git commit -am "build(lsp): upgrade flux-lsp to ${VERSION}"
hub pull-request -p -m "build(lsp): upgrade flux-lsp to ${VERSION}"

cd $TEMPDIR/vsflux
git checkout -b build/update-lsp-to-${VERSION}
npm i --save @influxdata/flux-lsp-node@${VERSION}
git commit -am "build(lsp): upgrade flux-lsp to ${VERSION}"
hub pull-request -p -m "build(lsp): upgrade flux-lsp to ${VERSION}"

rm -rf $TEMPDIR