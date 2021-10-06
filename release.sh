#!/bin/bash

# This script will check the state of the main branch of flux-lsp for
# conditions that would allow for a release to occur. If those conditions
# are met, a signed tag is created and *pushed to github* where the CI
# will take over and publish the extension.
#
# WARNING: This script will *push directly to master*. Please make sure
# you understand the contents of this script and the consequences of its
# execution before running it.
set -e

if [[ ! $(command -v hub) ]]; then
	echo "Please install the hub tool and re-run."
	exit 1
fi

if [[ ! $(cargo bump -v) ]]; then
    echo "Please install cargo bump and re-run"
    exit 1
fi

TEMPDIR=$(mktemp -d -t flux-release.XXXX)
echo "Using fresh install in $TEMPDIR"
cd $TEMPDIR
git clone git@github.com:influxdata/flux-lsp.git > /dev/null 2>&1
cd $TEMPDIR/flux-lsp

cargo bump patch --git-tag
git push

new_version=`grep "^version" Cargo.toml | awk '{print $3}' | awk -F'"' '{print $2}'`
previous_version=`git describe --abbrev=0 ${new_version}^`
commits=`git log --pretty=oneline ${previous_version}...${new_version} | tail -n +2 | awk '{$1="-"; print }'`
hub release create $new_version -m "Release $new_version

${commits}"
echo "$new_version tagged and released"

rm -rf $TEMPDIR
