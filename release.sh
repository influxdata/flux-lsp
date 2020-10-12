#!/bin/bash

which hub &> /dev/null
if [[ @? != 0 ]]; then
	echo "Please install the hub command line tool before running this script."
	echo "https://github.com/github/hub"
fi

current_branch=$(git branch --show-current)
if [[ $current_branch != "master" ]]; then
	echo "You are not on the master branch. Aborting."
	exit 1
fi


git_changes=$(git status -s | wc -l)
if [[ $git_changes != 0 ]]; then
	echo "You have modified the master branch."
	echo "Please revert or move your changes to another branch before running this script."
	exit 1
fi

git fetch
ahead=$(git status -sb | grep ahead -c)
if [[ $ahead != 0 ]]; then
	echo "Your local master branch is ahead of the remote master branch. Exiting."
	exit 1
fi

release_type=$1
if [[ $release_type != "minor" && $release_type != "patch" && $release_type != "major" ]]; then
	echo "Invalid argument: $release_type"
	exit 1
fi

git pull origin master

version=v$(cat Cargo.toml | grep -Po -m 1 '\d+\.\d+\.\d+')
cargo install -q cargo-bump && cargo bump $release_type
new_version=v$(cat Cargo.toml | grep -Po -m 1 '\d+\.\d+\.\d+')

echo "Cutting $release_type release"
echo "$version -> $new_version"

git add .
git commit -m "build(release): Release $new_version"
git tag -a $new_version HEAD -m "Release $new_verion"
git push origin master --follow-tags

hub release create $new_version -m "Release $new_version" -e
