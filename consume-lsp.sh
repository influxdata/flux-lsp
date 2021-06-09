#!/bin/bash

# This script should only be run after the Flux release process has successfully
# triggered a version bump in the LSP. Once the version bump PR has merged this
# script will handle the rest of the release process for the LSP, with minimal
# babysitting by an engineer.

# Some pre-requisites:
# - Install the hub cli, and make sure you can use it to interact with github
#   (this may require generating a personal access token via the github ui)
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

TEMPDIR=$(mktemp -d -t lsp-release.XXXX)
echo "Using ${TEMPDIR}"
cd $TEMPDIR

function tmp_clone() {
	git clone git@github.com:influxdata/$1.git &> /dev/null
	echo "$(pwd)/$1"
}

LSP_DIR=$(tmp_clone flux-lsp)
UI_DIR=$(tmp_clone ui)

cd $LSP_DIR
lsp_version=$(grep -Eom 1 "([0-9]{1,}\.)+[0-9]{1,}" Cargo.toml)

if ! hub release show $lsp_version &> /dev/null
then
	echo "Release v$lsp_version already exists"
	exit 1
fi

echo "Cutting release for Flux LSP v$lsp_version"
make tag-release

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
	cd $TEMPDIR
	VSFLUX_DIR=$(tmp_clone vsflux)
	CLI_DIR=$(tmp_clone flux-lsp-cli)

	cd $VSFLUX_DIR
	make patch-version

	cd $CLI_DIR
	make patch-version

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
	make tag-release

	cd $CLI_DIR
	git checkout master
	git pull
	make tag-release
else
	echo "Not the first week of the month. Skipping releases for vsflux and flux-lsp-cli."
fi

rm -rf $TEMPDIR
