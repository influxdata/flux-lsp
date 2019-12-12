build:
	cargo build

install:
	cargo install --path . --force

test:
	cargo test -- --test-threads=1

manual-test: test install
	vim me.flux

wasm: build
	./build.sh
