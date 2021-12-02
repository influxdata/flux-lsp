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
if [[ ! -f $HOME/.config/hub ]]; then
    echo "Please authenticate your hub command. See https://github.com/github/hub/issues/2655#issuecomment-735836048"
    exit 1
fi
if [[ ! $(cargo bump -v) ]]; then
    echo "Please install cargo bump and re-run: `cargo install cargo-bump`"
    exit 1
fi

TEMPDIR=$(mktemp -d -t flux-release.XXXX)
echo "Using fresh install in $TEMPDIR"
cd $TEMPDIR
git clone git@github.com:influxdata/flux-lsp.git > /dev/null 2>&1
cd $TEMPDIR/flux-lsp

# Bump version
cargo bump patch
cargo check
new_version=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)

# Commit and tag release
git add Cargo.toml
git add Cargo.lock
# Note: Using an annotated tag (-a) is important so that we can reliably find
# the previous version tag.
git tag -a -m "$new_version" "$new_version"
git commit -m "$new_version"
git push


previous_version=`git describe --abbrev=0 ${new_version}^`
commits=`git log --pretty=oneline ${previous_version}...${new_version} | tail -n +2 | awk '{$1="-"; print }'`
hub release create $new_version -m "Release $new_version

${commits}"
echo "$new_version tagged and released"

rm -rf $TEMPDIR
