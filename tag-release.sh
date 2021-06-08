#!/bin/bash

hub_installed=$(command -v hub)
if [[ ! $hub_installed ]]; then
	echo "Please install the hub command line tool before running this script."
	echo "https://github.com/github/hub"
	exit 1
fi

new_version=v$(grep -Eom 1 "([0-9]{1,}\.)+[0-9]{1,}" Cargo.toml)

git tag -a -s $new_version -m "Release $new_version"
git push origin master $new_version

flux_version=$(grep -m 1 'flux = ' Cargo.toml | grep -Eom 1 "([0-9]{1,}\.)+[0-9]{1,}")

hub release create $new_version -m "Release $new_version

- Upgrade to [Flux $flux_version](https://github.com/influxdata/flux/releases/tag/$flux_version)" -e
