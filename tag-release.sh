#!/bin/bash

hub_installed=$(command -v hub)
if [[ ! $hub_installed ]]; then
	echo "Please install the hub command line tool before running this script."
	echo "https://github.com/github/hub"
	exit 1
fi

new_version=v$(cat Cargo.toml | grep -Po -m 1 '\d+\.\d+\.\d+')

git tag -a -s $new_version -m "Release $new_version"
git push origin master $new_version

flux_version=$(cat Cargo.toml | grep -P -m 1 'flux = *' | grep -Po 'v\d+\.\d+\.\d+')

hub release create $new_version -m "Release $new_version

- Upgrade to [Flux $flux_version](https://github.com/influxdata/flux/releases/tag/$flux_version)" -e
