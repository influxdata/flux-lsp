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

if [[ "${DEBUG:-0}" == "1" ]]; then
    set -x
fi

# Controls how the version is bumped.
# Set INCREMENT to one of: major, minor or patch
# Defaults to patch if unset.
INCREMENT=${INCREMENT:-patch}

if [[ ! $INCREMENT =~ (patch)|(minor)|(major) ]]
then
    echo "Increment must be one of major, minor or patch"
    exit 1
fi

if [[ ! $(command -v hub) ]]; then
    echo "Please install the hub tool and re-run."
    exit 1
fi
if [[ ! -f $HOME/.config/hub && "${GITHUB_TOKEN}" == "" ]]; then
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
if [[ $(ssh -T git@github.com 2>&1 > /dev/null) ]]; then
  git clone git@github.com:influxdata/flux-lsp.git > /dev/null 2>&1
else
  git clone https://github.com/influxdata/flux-lsp.git > /dev/null 2>&1
fi

cd $TEMPDIR/flux-lsp
if [[ ! $(hub ci-status HEAD) ]]; then
    echo "Build status on master is either incomplete or failing. Please try ag ain after build status is complete."
    exit 1
fi

# Bump version
cargo bump $INCREMENT
cargo check
new_version=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)

# Commit and tag release
git add Cargo.toml
git add Cargo.lock
git commit -m "release: $new_version"
# Note: Using an annotated tag (-a) is important so that we can reliably find
# the previous version tag.
git tag -a -m "$new_version" "$new_version"
git push


previous_version=`git tag --sort=-creatordate | sed -n '2 p'`
# The tail step here ignores the commit that is the release, so we don't have a changelog that also
# contains, e.g. "release: 0.10.55". We already know it's a release, that's why we're constructing release
# notes.
commits=`git log --pretty=oneline ${previous_version}..${new_version} | tail -n +2 | awk '{$1="-"; print }'`
hub release create $new_version -m "Release $new_version

${commits}"
echo "$new_version tagged and released"

rm -rf $TEMPDIR
