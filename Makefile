build:
	cargo build

install:
	cargo install --path . --force

lint:
	cargo fmt --all -- --check
	cargo clippy --all -- -D warnings

test:
	cargo test $(tests) -- --nocapture

clean-wasm:
	rm -rf pkg-node
	rm -rf pkg-browser

wasm: clean-wasm
	AR=llvm-ar ./wasm-build.sh

publish:
	./publish.sh

install-wasm-pack:
	curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
