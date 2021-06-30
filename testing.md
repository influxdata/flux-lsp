Testing local wasm builds

Requirements

- Docker
- Node (> 12)
- Rust (> 1.40)

1.
Pull the following repos
  - github.com/influxdata/vsflux
  - github.com/influxdata/flux-lsp-cli
  - github.com/influxdata/flux-lsp

2.
CD into the flux-lsp repo and run `make wasm`

3.
CD into the flux-lsp-cli repo and update `package.json` and change the dependency of `@influxdata/flux-lsp-node` to `file: <full path to flux-lsp>/pkg-node`
run `npm install`

3.
CD into the vsflux repo and update `package.json` and change the dependency of `@influxdata/flux-lsp-cli` to `file: <full path to flux-lsp-cli>`
run `npm install`


4. open vsflux in vscode and run
