# Flux LSP

An implementation of the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) for the [Flux language](https://github.com/influxdata/flux).

# Install

```
cargo install --git git@github.com:influxdata/flux-lsp.git
```

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
