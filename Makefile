build:
	cargo build

install:
	cargo install --path . --force

lint:
	cargo fmt --all -- --check
	cargo clippy --all -- -D warnings

test:
	@echo "------------------------------------------------------------------"
	@echo "tests arg can be used to run specific tests"
	@echo ""
	@echo "for example:"
	@echo "  make test tests=Find_references::test_returns_correct_response"
	@echo ""
	@echo "if no tests arg provided will run entire suite"
	@echo "------------------------------------------------------------------"
	@echo ""
	@echo ""

	cargo test $(tests) -- test  --nocapture

manual-test: test install
	vim me.flux

clean-wasm:
	rm -rf pkg-node
	rm -rf pkg-browser

wasm: clean-wasm build
	./build.sh

wasm-local: clean-wasm
	AR=llvm-ar ./wasm-build.sh

publish:
	./publish.sh

install-wasm-pack:
	curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

patch-release:
	./release.sh patch

minor-release:
	./release.sh minor
