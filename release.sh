#!/bin/bash

current_branch=$(git branch --show-current)
git_changes=$(git status -s | wc -l)
if [[ $git_changes != 0 ]]; then
	if [[ $current_branch == "master" ]]; then
		echo "You have modified the master branch."
		echo "Please revert or move your changes to another branch before running this script."
	else
		echo "Please commit or stash your changes before running this script"
	fi
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

read -ep "Are you sure you want to cut a $release_type release? [y/N] " continue
if [[ $continue != "yes" && $continue != "y" && $continue != "Yes" && $continue != "YES" ]]; then
	exit
fi

git checkout master
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

# Requires the hub CLI tool to be installed
hub release create $new_version -m "Release $new_version" -e
