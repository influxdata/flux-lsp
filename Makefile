build:
	cargo build

install:
	cargo install --path . --force

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

	cargo test $(tests) -- test  --test-threads=1 --nocapture

manual-test: test install
	vim me.flux

clean-wasm:
	rm -rf pkg-node
	rm -rf pkg-browser

wasm: clean-wasm build
	./build.sh

publish: build
	./publish.sh
