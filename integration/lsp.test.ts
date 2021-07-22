interface LSPRequest {
    method: string
    params?: object[] | object
}

interface LSPResponse {
    result?: string | number | boolean | object | null
}

function buildRequest(request: LSPRequest): string {
    const json = JSON.stringify({...request, jsonrpc: "2.0", id: 1});
    return `Content-Length: ${json.length}\r\n\r\n${json}`;
}

function parseResponse(data: string): LSPResponse {
    const json: object = JSON.parse(data.split('\r\n')[2]);
    return { result: json["result"] };
}

describe('LSP Server integration tests', () => {
    let server;

    beforeAll(async () => {
        const { Server } = await import('@influxdata/flux-lsp-node');
        server = new Server(false, false);
    });


    it('initializes', async () => {
        const expected = {
            "result": {
                "capabilities": {
                    "textDocumentSync":1,
                    "referencesProvider":true,
                    "definitionProvider":true,
                    "renameProvider":true,
                    "foldingRangeProvider":true,
                    "documentSymbolProvider":true,
                    "documentFormattingProvider":true,
                    "completionProvider": {
                        "resolveProvider":true,
                        "triggerCharacters": [".",":","(",",","\""]
                    },
                    "signatureHelpProvider": {
                        "triggerCharacters": ["("],
                        "retriggerCharacters": ["("]
                    },
                    "hoverProvider":true
                },
                "serverInfo": {
                    "name": "flux-lsp",
                    "version": "1.0"
                }
            }
        };

        const request = {method: "initialize"};

        const response = await server.process(buildRequest(request));
        const error = response.get_error();
        expect(error).toBe(undefined);
        const message = parseResponse(response.get_message());
        expect(message).toStrictEqual(expected);
    });

    it('formats', async () => {

        const expected = {
            result: [{
                newText: "from(bucket: \"my-bucket\") |> group() |> last()",
                range: {
                    start: {
                        character: 0,
                        line: 0
                    },
                    end: {
                        character: 41,
                        line: 0
                    }
                }
            }],
        };

        const open_request = {
            method: "textDocument/didOpen",
            params: {
                textDocument: {
                    uri: "file:///path/to/file.flux",
                    languageId: "flux",
                    version: 1,
                    text: `from(bucket:"my-bucket")|>group()|>last()`
                }
            }
        };

        const open_response = await server.process(buildRequest(open_request));
        const open_error = open_response.get_error();
        expect(open_error).toBe(undefined);

        const request = {
            method: "textDocument/formatting",
            params: {
                textDocument: {
                    uri: "file:///path/to/file.flux"
                },
                options: {
                    tabSize: 4,
                    insertSpaces: true,
                },
                work_done_progress_params: {},
            }
        };
        const response = await server.process(buildRequest(request));
        const error = response.get_error();
        expect(error).toBe(undefined);

        const message = parseResponse(response.get_message());
        expect(message).toStrictEqual(expected);
    });

    /* This test doesn't assert anything. It merely calls out an API that is used
     * by downstream consumers, asserting that it exists.
     */
    it('registers callbacks', async () => {
        server.register_buckets_callback(() => { console.log("buckets callback"); });
        server.register_measurements_callback(() => { console.log("measurements callback"); });
        server.register_tag_keys_callback(() => { console.log("tag keys callback"); });
        server.register_tag_values_callback(() => { console.log("tag values callback"); });
    });
});
