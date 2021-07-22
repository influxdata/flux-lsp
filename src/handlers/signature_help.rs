use crate::cache::Cache;
use crate::handlers::{find_node, Error, RequestHandler};
use crate::protocol::{PolymorphicRequest, Request, Response};
use crate::shared::signatures::FunctionSignature;
use crate::shared::RequestContext;
use crate::stdlib::{get_stdlib_functions, BUILTIN_PACKAGE};
use crate::visitors::semantic::functions::FunctionFinderVisitor;
use crate::visitors::semantic::utils::{
    create_completion_package, create_semantic_package,
};

use flux::semantic::nodes::Expression;
use flux::semantic::walk::{walk, Node};

use std::rc::Rc;

use lsp_types as lsp;

fn create_signature_information(
    fs: FunctionSignature,
) -> lsp::SignatureInformation {
    lsp::SignatureInformation {
        label: fs.create_signature(),
        parameters: Some(fs.create_parameters()),
        documentation: None,
        active_parameter: None,
    }
}

#[derive(Default)]
pub struct SignatureHelpHandler {}

pub fn find_stdlib_signatures(
    name: String,
    package: String,
) -> Vec<lsp::SignatureInformation> {
    get_stdlib_functions()
        .into_iter()
        .filter(|x| x.name == name && x.package_name == package)
        .map(|x| {
            x.signatures()
                .into_iter()
                .map(create_signature_information)
        })
        .fold(vec![], |mut acc, x| {
            acc.extend(x);
            acc
        })
}

fn find_user_defined_signatures(
    pos: lsp::Position,
    uri: lsp::Url,
    name: String,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<lsp::SignatureInformation>, String> {
    let pkg = create_completion_package(uri, pos, ctx, cache)?;
    let mut visitor = FunctionFinderVisitor::new(pos);

    walk(&mut visitor, Rc::new(Node::Package(&pkg)));

    let state = visitor.state.borrow();
    let functions = (*state).functions.clone();

    Ok(functions
        .into_iter()
        .filter(|x| x.name == name)
        .map(|x| {
            x.signatures()
                .into_iter()
                .map(create_signature_information)
        })
        .fold(vec![], |mut acc, x| {
            acc.extend(x);
            acc
        }))
}

fn find_signatures(
    request: Request<lsp::SignatureHelpParams>,
    ctx: RequestContext,
    cache: &Cache,
) -> Result<Vec<lsp::SignatureInformation>, String> {
    let mut result = vec![];

    if let Some(params) = request.params {
        let pos = params.text_document_position_params.position;
        let uri = lsp::Url::parse(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str(),
        )
        .unwrap();
        let pkg = create_semantic_package(uri.clone(), cache)?;
        let node_result = find_node(Node::Package(&pkg), pos);

        if let Some(node) = node_result.node {
            if let Node::CallExpr(call) = node.as_ref() {
                let callee = call.callee.clone();

                if let Expression::Member(me) = callee.clone() {
                    let name = me.property.clone();
                    if let Expression::Identifier(ident) =
                        me.object.clone()
                    {
                        result.extend(find_stdlib_signatures(
                            name, ident.name,
                        ));
                    }
                } else if let Expression::Identifier(ident) = callee {
                    result.extend(find_stdlib_signatures(
                        ident.name.clone(),
                        BUILTIN_PACKAGE.to_string(),
                    ));
                    result.extend(find_user_defined_signatures(
                        pos, uri, ident.name, ctx, cache,
                    )?);
                }
            }
        }
    }

    Ok(result)
}

#[async_trait::async_trait]
impl RequestHandler for SignatureHelpHandler {
    async fn handle(
        &self,
        prequest: PolymorphicRequest,
        ctx: RequestContext,
        cache: &Cache,
    ) -> Result<Option<String>, Error> {
        let req: Request<lsp::SignatureHelpParams> =
            Request::from_json(prequest.data.as_str())?;

        let sh = lsp::SignatureHelp {
            signatures: find_signatures(req.clone(), ctx, cache)?,
            active_signature: None,
            active_parameter: None,
        };

        let resp = Response::new(req.id, Some(sh));
        let json = resp.to_json()?;

        Ok(Some(json))
    }
}
