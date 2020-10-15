# How to Update the LSP and its Consumers

## Prerequisites

Install the following

- Any stable release of Rust >= 1.47
- `npm`
- `wasm-pack`

To install `wasm-pack`, run `make install-wasm-pack` from the project root of `flux-lsp`.

Pull down the most recent versions of the following repos:
- [ `flux-lsp` ](https://www.github.com/influxdata/flux-lsp)
- [ `flux-lsp-cli` ](https://www.github.com/influxdata/flux-lsp-cli)
- [ `flux-lsp` ](https://www.github.com/influxdata/vsflux)
- [ `influxdb` ](https://www.github.com/influxdata/influxdb)

## Testing Locally

`cd` into `flux-lsp`.

If you want to test the LSP with a new Flux release, open `Cargo.toml` and look for the `flux` dependency. Replace the `tag` value with the tag for the current Flux release. Run `cargo test` and confirm that all tests pass.

Then, navigate to `ui` directory in `influxdb`, and open `package.json`. Find the `@influxdata/flux-lsp-browser` dependency, and replace the version number with `file:`, followed by the full file path to the `pkg-browser` directory in your local copy of `flux-lsp`.

Example:

```json
"dependencies": {
    "@influxdata/flux-lsp-browser": "file: /home/janedoe/projects/flux-lsp/pkg-browser"
}
```

Run `yarn add`, then navigate back to the project root and run `make test-js`. Confirm that the tests pass.

### Running your local LSP changes in VS Code (Optional)

After confirming that all the tests pass, run `make wasm` to compile with a docker container, or `make wasm-local` to compile locally. This will create two directories: `pkg-node` and `pkg-browser`. Each is a ready-to-publish npm package for different wasm compilation targets.

Navigate to the `vsflux` repo, and open `package.json`. Find the `@influxdata/flux-lsp-node` dependency, and replace the version number with `file:`, followed by the full file path to the `pkg-node` directory in your local copy of `flux-lsp`. It should look something like:

```json
"dependencies": {
    "@influxdata/flux-lsp-node": "file: /home/janedoe/projects/flux-lsp/pkg-node"
}
```

Then, from the root of `vsflux`, run `npm install`. 

Finally, open up the `vsflux` project in VS Code, click on the `Run` tab in the sidebar, and then click the green arrow at the top of the pane. This should open up a new VS Code window that is running your local version of the extension, rather than the one available on the marketplace. Confirm that any recent changes are working as expected.

## Cutting a release

When the `master` branch of the `flux-lsp` is ready for a release, pull down the latest changes from GitHub, and do the following:

### Increment the version

Bump the version number listed near the top of `Cargo.toml`. You can do this by hand, but it is better to use a tool like [`cargo bump`](https://github.com/wraithan/cargo-bump). You can install it through cargo with 
```
cargo install cargo-bump
```

Once installed, use one of the following commands to programatically bump the version:

- `cargo bump patch` for a patch release (example: `0.5.20 -> 0.5.21`)
- `cargo bump minor` for a minor release (example: `0.5.xx -> 0.6.0`)
- `cargo bump major` for a major release (example: `0.5.xx -> 1.0.0`)

*Note: most of the time, you will want to do a patch release*

### Commit, tag, and push

Commit the `Cargo.toml` change to the ***master branch*** of `flux-lsp`. **It is very important that no other changes are included in this commit, as it will be pushed directly to master.**

Add a tag to that commit that consists of the version number prepended with a `'v'` (example: `v0.5.20`). Git will prompt you to include a message with your tag, which should just be `"Release <tag-name>"`.

As an example, if the new version was version `0.5.21`, you could accomplish all of this with the following command:

```
git tag -a v0.5.21 $(git rev-parse HEAD) -m "Release v0.5.21"
```

Push the commit along with its tag by running `git push --follow-tags`

Confirm that the both of the following have occurred:

1. The tag has been pushed to the master branch of the GitHub repo.

2. CircleCI has detected the version tag, and has triggered a job that will build the [ `flux-lsp-node` ](https://www.npmjs.com/package/@influxdata/flux-lsp-node) and [ `flux-lsp-browser` ](https://www.npmjs.com/package/@influxdata/flux-lsp-browser) packages and deploy them to `npm`.

The last thing to do for the `flux-lsp` repo is to cut a release on GitHub. Go to the [GitHub repo](https://www.github.com/influxdata/flux-lsp) and click on the `Releases` link and draft a new release with the tag you just pushed. The title should be the same as the messaage you included with your tag (e.g. `Release 0.5.21`), and the description should include a brief summary of the changes made since the last release.

### Update the CLI and the VS Code Extension

In both `flux-lsp-cli` and `vsflux`, update the `flux-lsp-node` dependency to the latest version in `package.json`, then run `npm install`. Commit the changes to a new branch, and open a pull request to `master`

The process for cutting a release for these repos should be virtually identical to cutting a release for `flux-lsp`. The only difference is that instead of using `cargo bump` to increment the version number, you should use `npm version`, followed by the type of release (major, minor, or patch), followed by the `--no-git-tag-version` flag. 

Example:

```
npm version patch --no-git-tag-version
```

Like `flux-lsp`, `flux-lsp-cli` and `vsflux` both have CircleCI jobs that will take care of deploying them once the version tag is detected. Still, you should confirm that the new versions have been deployed to [ NPM ](https://www.npmjs.com/package/@influxdata/flux-lsp-cli) and the [ VS Code Extension Marketplace ](https://marketplace.visualstudio.com/items?itemName=influxdata.flux).

Again, other than those minor details, the release process should be identical to that of `flux-lsp`.

### Update InfluxDB

The last thing to do is to pull down a fresh copy of [`influxdb`](https://github.com/influxdata/influxdb), open up `ui/package.json`, and update the `flux-lsp-browser` dependency to the latest version. Run `yarn add`, commit the changes, run `make test` to confirm nothing breaks, and open a PR into `master`.
