Flux Composition
==

The composition capabilities of the flux LSP server enable clients to create a query builder experience without deep knowledge of the flux internals. Composition will create a "Composition-owned" statement in a given flux file, and will be able to add/remove filter calls to that statement. The composition rules apply as follows:

  - Composition must never generate a query that cannot be executed.
  - Composition should never generate a query that a user with sufficient flux knowledge would never write, e.g. `filter(fn: (r) => r.foo == "bar") |> filter(fn: (r) => r.foo == "bar")`
  - Composition should never generate a query that results in immediate diagnostic messages.
  - In the event of conflicts or logical ambiguity, Composition should default to erroring rather than introducing complex assumption logic to attempt recovery.

Given these rules, this document will not attempt to document the resulting flux code generated from each of these requests, as the resulting flux could change as requirements evolve and the corresponding flux would become outdated.

Composition Initialize
--

The composition initialize request is sent from the client to the server to identify a file that will be used to compose queries. This call must be made before any other Composition requests, otherwise those requests will error. It will result in a `workspace/applyEdit` request from the server to the client with the associated changes to the file. The request can also be used to reset the composition query state by calling it again, and can be considered idempotent.

*Request*
  - method: `fluxComposition/initialize`
  - params: `CompositionInitializeParams`

```
interface CompositionInitializeParams {
    /*
     * The text document.
     */
    text_document: TextDocumentIdentifier;

    /*
     * The bucket to initialize the composition query.
     */
    bucket: string;

    /*
     * The measurement to initialize the composition query.
     */
    measurement?: string;
}
```

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the initialize request.

Add Measurement Filter
--

The add measurement filter request is sent from the client to the server to add a measurement filter. If a measurement is not already specified, e.g. it was not specified in the initialization and not called previously, it will result in a `workspace/applyEdit` request from the server to the client with the associated filter addition. If a measurement filter already exists, an error will be reported in the form of a `window/showMessage` request from the server to the client with the associated information.

*Request*
  - method: `fluxComposition/addMeasurementFilter`
  - params: `ValueFilterParams`

```
interface ValueFilterParams {
    /*
     * The text document.
     */
    text_document: TextDocumentIdentifier;

    /*
     * The value to filter.
     */
    value: string;
}
```

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Add Field Filter
--

The add field filter request is sent from the client to the server to add a field filter. If the specific field filter does not already exist, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the field filter already exists, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/addFieldFilter`
  - params: `ValueFilterParams`

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Add Tag Filter
--

The add tag filter request is sent from the client to the server to add a tag filter. If the specific tag filter does not already exist, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the tag filter already exists, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/addTagFilter`
  - params: `ValueFilterParams`

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Add Tag Value Filter
--

The add tag value filter request is sent from the client to the server to add a tag value filter. If the specific tag value filter does not already exist, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the tag value filter already exists, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/addTagValueFilter`
  - params: `TagValueFilterParams`

```
interface TagValueFilterParams {
    /*
     * The text document.
     */
    text_document: TextDocumentIdentifier;

    /*
     * The tag name to filter.
     */
    tag: string;

    /*
     * The value of the tag to filter.
     */
    value: string;
}
```

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Remove Field Filter
--

The remove field filter request is sent from the client to the server to remove an existing field filter. If the specific field filter exists, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the field filter does not exist, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/removeFieldFilter`
  - params: `ValueFilterParams`

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Remove Tag Filter
--

The remove tag filter request is sent from the client to the server to remove a tag filter. If the specific tag filter already exists, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the tag filter does not exist, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/removeTagFilter`
  - params: `ValueFilterParams`

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.


Remove Tag Value Filter
--

The remove tag value filter request is sent from the client to the server to remove a tag value filter. If the specific tag value filter already exists, the request will result in a `workspace/applyEdit` request from the server to the client with the associated changes. If the tag value filter does not already exist, an error will be reported in the form of a `window/showMessage` request from the server to the client.

*Request*
  - method: `fluxComposition/removeTagValueFilter`
  - params: `TagValueFilterParams`

*Response*
  - result: `null`
  - error: code and message set in case an exception happens during the request.