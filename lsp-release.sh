#!/bin/bash

# This script should only be run after the Flux release process has successfully
# triggered a version bump in the LSP. Once the version bump PR has merged, this
# script will handle the rest of the release process for the LSP, with minimal
# babysitting by an engineer.

# Some pre-requisites:
# - Install the hub cli, and make sure you can use it to interact with GitHub
#   (this may require generating a personal access token via the GitHub web ui)
# - Install npm & yarn package managers
# - Install jq
# - Have a personal GPG key set up and imported into your GitHub account
# - Have write access to the influxdata/ui repo

set -e

if ! command -v hub &> /dev/null
then
	echo "hub is not installed. exiting"
	exit 1
fi

if ! command -v jq &> /dev/null
then
	echo "jq is not installed. exiting"
	exit 1
fi

if ! command -v npm &> /dev/null
then
	echo "npm is not installed. exiting"
	exit 1
fi

if ! command -v yarn &> /dev/null
then
	echo "yarn is not installed. exiting"
	exit 1
fi

# Some helper functions
function version() {
	grep -Eom 1 "([0-9]{1,}\.)+[0-9]{1,}" $1
}

function bump_npm_version() {
	release_type=$1
	if [[ $release_type != "patch" && $release_type != "minor" ]]; then
		echo "Invalid argument: $release_type"
		exit 1
	fi
	npm version $release_type --no-git-tag-version
	npm install
	v=v$(version package.json)
	
	branch="bump-$v"

	git checkout -B $branch
	echo "Using branch \`$branch\`"

	npm add @influxdata/flux-lsp-node
	git commit -am "build: Release $v"
	git push -u origin $branch

	hub pull-request -o \
		-m "build: Release $v" \
		-m "- Bump version to $v
- Import latest version of flux-lsp-node" &> /dev/null &
}

function tag_npm_release() {
	v=v$(version package.json)
	git tag -a -s $v -m "Release $v"
	git push origin master $v

	lsp_version=v$(grep -m 1 '"@influxdata/flux-lsp-node":' package.json | version)
	hub release create $v -m "Release $v

- Upgrade to [Flux LSP v$lsp_version](https://github.com/influxdata/flux-lsp/releases/tag/v$lsp_version)" -e
}

# Start script
TEMPDIR=$(mktemp -d -t lsp-release.XXXX)
echo "Using ${TEMPDIR}"

function tmp_clone() {
	cd $TEMPDIR
	git clone git@github.com:influxdata/$1.git &> /dev/null
	echo "$(pwd)/$1"
}

LSP_DIR=$(tmp_clone flux-lsp)
UI_DIR=$(tmp_clone ui)

function tag_lsp_release() {
	v=v`version $LSP_DIR/Cargo.toml`
	git tag -a -s $v -m "Release $v"
	git push origin master $v
	flux_version=$(grep -m 1 'flux = ' $LSP_DIR/Cargo.toml | version)
	hub release create $v -m "Release $v

	- Upgrade to [Flux $flux_version](https://github.com/influxdata/flux/releases/tag/$flux_version)" -e
}

cd $LSP_DIR
lsp_version=$(version Cargo.toml)

if ! hub release show $lsp_version &> /dev/null
then
	echo "Release v$lsp_version already exists"
	exit 1
fi

echo "Cutting release for Flux LSP v$lsp_version"
tag_lsp_release

echo "Waiting for the new release to hit the NPM registry..."
echo -e "This may take up to 30 minutes\n"
echo "Once the release is up, this script will open a new browser tab"
echo "with a PR into the UI repo, importing the new version of the LSP."
while true; do
	npm_node_version=$(npm search --json @influxdata/flux-lsp-node | jq -r '.[0].version')
	npm_browser_version=$(npm search --json @influxdata/flux-lsp-browser | jq -r '.[0].version')
	[[ $npm_node_version == $lsp_version ]] && [[ $npm_browser_version == $lsp_version ]] && break
	sleep 30
done

branch_name="build/lsp-$lsp_version"

function uipr() {
	git checkout -b $branch_name
	yarn add @influxdata/flux-lsp-browser
	git commit -am "build(lsp): Upgrade flux-lsp-browser to v$lsp_version"
	git push -u origin $branch_name

	hub pull-request -o \
		-m "build(lsp): Upgrade to flux-lsp-browser v$lsp_version" \
		-m "Upgrade flux-lsp-browser to [v$lsp_version](https://github.com/influxdata/flux-lsp/releases/tag/v$lsp_version)" &> /dev/null &
}

cd $UI_DIR
uipr

# If it's the first week of the month, cut releases for vsflux and flux-lsp-cli
wom=`expr $(expr $(date +%-d) - 1) / 7 + 1`
if [[ $wom == 1 ]]; then
	echo "It's the first week of the month!"
	echo "Cutting releases for vsflux and flux-lsp-cli..."
	VSFLUX_DIR=$(tmp_clone vsflux)
	CLI_DIR=$(tmp_clone flux-lsp-cli)

	cd $VSFLUX_DIR
	bump_npm_version patch

	cd $CLI_DIR
	bump_npm_version patch

	echo ""
	echo "Wait for the vsflux and flux-lsp-cli PRs to merge,"
	echo "then type 'release' to continue. Doing so will tag releases for both repos."
	echo -e "\nAlternatively, you can safely CTRL-C out of this script and tag the releases yourself"
	while [ true ] ; do
		read -t 10000 -n 7 input
		if [[ $input == "release" ]]; then
			echo ""
			break
		else
			echo ""
			echo "try again"
			continue
		fi
	done

	echo "Tagging releases..."

	cd $VSFLUX_DIR
	git checkout master
	git pull
	tag_npm_release

	cd $CLI_DIR
	git checkout master
	git pull
	tag_npm_release
else
	echo "Not the first week of the month. Skipping releases for vsflux and flux-lsp-cli."
fi

rm -rf $TEMPDIR
