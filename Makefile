build:
	cargo build

install:
	cargo install --path . --force

test: install
	vim me.flux
