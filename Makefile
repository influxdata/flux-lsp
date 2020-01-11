build:
	cargo build

install:
	cargo install --path . --force

test:
	cargo test -- --test-threads=1 --nocapture

manual-test: test install
	vim me.flux

wasm: build
	./build.sh

publish: build
	./publish.sh
