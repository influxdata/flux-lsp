# Flux LSP

[![LICENSE](https://img.shields.io/github/license/influxdata/flux-lsp.svg)](https://github.com/influxdata/flux-lsp/blob/master/LICENSE)
[![Slack Status](https://img.shields.io/badge/slack-join_chat-white.svg?logo=slack&style=social)](https://www.influxdata.com/slack)

An implementation of the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) for the [Flux language](https://github.com/influxdata/flux).

# LSP Development

* LSP development requires rust version of 1.40.0 or newer.
* run tests with `make test`

# Installing command line server

```
npm i -g @influxdata/flux-lsp-cli
```

This will allow you to run an LSP instance with the command `flux-lsp`

# Vim setup

There are a lot of plugins that are capable of running language servers. This section will cover the one we use or know about.

In any case, you need to recognize the `filetype`. This is done looking at the file extension, in our case `.flux`. You should place this in your `vimrc` file:

```vimrc
" Flux file type
au BufRead,BufNewFile *.flux		set filetype=flux
```

### with vim-lsp
Requires [vim-lsp](https://github.com/prabirshrestha/vim-lsp)

in your .vimrc

```vimrc
let g:lsp_diagnostics_enabled = 1

if executable('flux-lsp')
    au User lsp_setup call lsp#register_server({
        \ 'name': 'flux lsp',
        \ 'cmd': {server_info->[&shell, &shellcmdflag, 'flux-lsp']},
        \ 'whitelist': ['flux'],
        \ })
endif

autocmd FileType flux nmap gd <plug>(lsp-definition)
```

### with vim-coc

Requires [vim-coc](https://github.com/neoclide/coc.nvim). `vim-coc` uses a `coc-settings.json` file and it is located in your `~/.vim` directory. In order to run the `flux-lsp` you need to add the `flux` section in the `languageserver`.

```json
{
  "languageserver": {
      "flux": {
        "command": "flux-lsp",
        "filetypes": ["flux"]
      }
  }
}
```
If you need to debug what flux-lsp is doing, you can configure it to log to `/tmp/fluxlsp`:

```json
{
  "languageserver": {
      "flux": {
        "command": "flux-lsp",
        "args": ["-l", "/tmp/fluxlsp"],
        "filetypes": ["flux"]
      }
  }
}
```

### with webpack

This package is distributed as a wasm file, and since wasm files cannot be included in the main bundle, you need to import the library a little differently:

```javascript
import('@influxdata/flux-lsp-browser')
    .then(({Server}) => {
        let server = new Server(false);
        // The LSP server is now ready to use
    });

```

Also ensure that the wasm file is not being parsed by any file loader plugins, as this will interfere with it's proper instantiation.


# Supported LSP features

- initialize
- shutdown
- textDocument/definition
- textDocument/didChange
- textDocument/didOpen
- textDocument/didSave
- textDocument/foldingRange
- textDocument/references
- textDocument/rename
- textDocument/completion
- textDocument/documentSymbol
- completionItem/resolve
