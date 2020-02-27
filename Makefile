build:
	cargo build

install:
	cargo install --path . --force

test:
	# tests arg can be used to run specific tests, for example:
	#	make test tests=Find_references::test_returns_correct_response
	# if no tests arg provided will run entire suite
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
