describe('LSP Server', () => {
    let server;

    beforeAll(async () => {
        const { initLog } = await import('@influxdata/flux-node');
        initLog();
    });

    beforeEach(async () => {
        const { Lsp } = await import('@influxdata/flux-node');
        server = new Lsp();
    });

    // helper function to send the initialize message
    const init = async (server) => {
        const request = '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "capabilities": {}}}';
        await server.send(request)
    }
    // helper function to shutdown and await the server
    const shutdown = async (server, runner) => {
        const shutdown = '{"jsonrpc": "2.0", "method": "shutdown", "id": 2}';
        const exit = '{"jsonrpc": "2.0", "method": "exit"}';
        await server.send(shutdown)
        await server.send(exit)
        await runner
    }

    it('responds to initialize request', async () => {
        const callback = jest.fn((message) => { });

        server.onMessage(callback);
        const runner = server.run();

        const request = '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "capabilities": {}}}';
        const response = JSON.parse(await server.send(request));
        expect(response).toBeDefined()
        expect(response).toHaveProperty('result.capabilities')

        await shutdown(server, runner);

        expect(callback).not.toHaveBeenCalled();
    });

    it('throws error on bad JSON message', async () => {
        const callback = jest.fn((message) => {
            console.log('callback', message);
        });

        server.onMessage(callback);
        const runner = server.run();
        await init(server);

        const request = 'Content-Length: 84\r\n\r\n{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "capabilities": {}}}';
        await expect(async () => {
            try {
                await server.send(request)
            } catch (e) {
                // jest can only assert on actual Errors being thrown not arbitrary expressions.
                // Catch anything and wrap it in an Error.
                throw new Error(e)
            }
        })
            .rejects
            .toThrow('failed to decode message JSON');

        await shutdown(server, runner);
        expect(callback).not.toHaveBeenCalled();
    });
    it('throws error on multiple runs', async () => {
        const callback = jest.fn((message) => { });

        server.onMessage(callback);
        const runner = server.run();
        await init(server);

        await expect(async () => {
            try {
                await server.run();
            } catch (e) {
                // jest can only assert on actual Errors being thrown not arbitrary expressions.
                // Catch anything and wrap it in an Error.
                throw new Error(e)
            }
        })
            .rejects
            .toThrow('run must not be called twice');

        await shutdown(server, runner);

        expect(callback).not.toHaveBeenCalled();
    });

    it('exits', async () => {
        // There are no explicit expectations defined in this test.
        // If the test runs without a timeout then it passed.
        const runner = server.run();
        await init(server);
        await shutdown(server, runner);
    });

    it('sends diagnostics', async () => {
        const callback = jest.fn((message) => {
            console.log('callback', message);
        });
        server.onMessage(callback);
        const runner = server.run();
        await init(server);

        const openRequest = '{"jsonrpc": "2.0", "id": 2, "method": "textDocument/didOpen", "params": { "textDocument": {"uri":"file:///main.flux","languageId":"flux","version":1,"text":"x ="}}}';
        await server.send(openRequest)

        await shutdown(server, runner);

        expect(callback).toHaveBeenCalled();
    });

    it('sends lists of diagnostics', async () => {
        const diagnostics = [];
        const callback = jest.fn((message) => {
            const diagnosticMessage = JSON.parse(message);
            diagnosticMessage.params.diagnostics.forEach(diagnostic => {
                diagnostics.push(diagnostic);
            });
        });
        server.onMessage(callback);
        const runner = server.run();
        await init(server);

        const openRequest = '{"jsonrpc": "2.0", "id": 2, "method": "textDocument/didOpen", "params": { "textDocument": {"uri":"file:///main.flux","languageId":"flux","version":1,"text":"from(bucket: x)"}}}';
        await server.send(openRequest)

        await shutdown(server, runner);

        expect(callback).toHaveBeenCalled();
        expect(diagnostics.length).toBe(1);
    });
});
describe('module', () => {

    it('can parse Flux source' , async () => {
        const { parse } = await import('@influxdata/flux-node');
        const ast = parse('x = 1');
        // expect some basic parts of this ast
        expect(ast).toBeDefined();
        expect(ast).toHaveProperty('type', 'File');
        expect(ast).toHaveProperty('location.source', 'x = 1');
        expect(ast).toHaveProperty('metadata', 'parser-type=rust');
        expect(ast).toHaveProperty('body');
    })

    it('can format Flux source' , async () => {
        const { parse, format_from_js_file } = await import('@influxdata/flux-node');
        const ast = parse('x = 1');
        // Change AST
        ast.body[0].init.value = '2';
        // Format into new source
        const src = format_from_js_file(ast);
        expect(src).toBe('x = 2');
    })
})
