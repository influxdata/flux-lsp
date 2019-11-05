# Flux LSP

Implementation of Language Server Protocol for the flux language

# Install

```
cargo install --git git@github.com:influxdata/flux-lsp.git
```

# Vim setup

Requires [vim-lsp](https://github.com/prabirshrestha/vim-lsp)

in your .vimrc

```

" Flux file type
au BufRead,BufNewFile *.flux		set filetype=flux

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
