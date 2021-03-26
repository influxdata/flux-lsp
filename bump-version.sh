#!/bin/bash

set -e

hub_installed=$(command -v hub)
if [[ ! $hub_installed ]]; then
	echo "Please install the hub command line tool before running this script."
	echo "https://github.com/github/hub"
	exit 1
fi

rg_installed=$(command -v rg)
if [[ ! $rg_installed ]]; then
	echo "Please install the hub command line tool before running this script."
	echo "https://github.com/github/hub"
	exit 1
fi

release_type=$1
if [[ $release_type != "patch" && $release_type != "minor" ]]; then
	echo "Invalid argument: $release_type"
	exit 1
fi

version=v$(rg -N -m 1 -r='$1' 'version = "(\d+\.\d+\.\d+)"' Cargo.toml)
cargo install -q cargo-bump && cargo bump $release_type
new_version=v$(rg -N -m 1 -r='$1' 'version = "(\d+\.\d+\.\d+)"' Cargo.toml)

branch_name=bump-$new_version

git checkout -B $branch_name
echo "Checking out branch \`$branch_name\`"

echo "Incrementing version"
echo "$version -> $new_version"

git add .
git commit -m "build: Release $new_version"
git push -u origin $branch_name

hub pull-request -o \
	-m "build: Bump to $new_version" \
	-m "Change version from $version to $new_version" &> /dev/null &
