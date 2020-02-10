use crate::handlers::{find_node, RequestHandler};
use crate::protocol::properties::Position;
use crate::protocol::requests::{
    PolymorphicRequest, Request, SignatureHelpParams,
};
use crate::protocol::responses::{
    Response, SignatureHelp, SignatureInformation,
};
use crate::shared::signatures::FunctionSignature;
use crate::stdlib::get_stdlib_functions;
use crate::visitors::semantic::functions::FunctionFinderVisitor;
use crate::visitors::semantic::utils::{
    create_completion_package, create_semantic_package,
};

use flux::semantic::nodes::Expression;
use flux::semantic::walk::{walk, Node};

use std::rc::Rc;

fn create_signature_information(
    fs: FunctionSignature,
) -> SignatureInformation {
    SignatureInformation {
        label: fs.create_signature(),
        parameters: Some(fs.create_parameters()),
        documentation: None,
    }
}

#[derive(Default)]
pub struct SignatureHelpHandler {}

fn find_stdlib_signatures(
    name: String,
    package: String,
) -> Vec<SignatureInformation> {
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
    pos: Position,
    uri: String,
    name: String,
) -> Result<Vec<SignatureInformation>, String> {
    let pkg = create_completion_package(uri, pos.clone())?;
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
    request: Request<SignatureHelpParams>,
) -> Result<Vec<SignatureInformation>, String> {
    let mut result = vec![];

    if let Some(params) = request.params {
        let pos = params.position;
        let uri = params.text_document.uri;
        let pkg = create_semantic_package(uri.clone())?;
        let node_result = find_node(Node::Package(&pkg), pos.clone());

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
                        "builtin".to_string(),
                    ));
                    result.extend(find_user_defined_signatures(
                        pos, uri, ident.name,
                    )?);
                }
            }
        }
    }

    Ok(result)
}

impl RequestHandler for SignatureHelpHandler {
    fn handle(
        &self,
        prequest: PolymorphicRequest,
    ) -> Result<Option<String>, String> {
        let req: Request<SignatureHelpParams> =
            Request::from_json(prequest.data.as_str())?;

        let sh = SignatureHelp {
            signatures: find_signatures(req.clone())?,
            active_signature: None,
            active_parameter: None,
        };

        let resp = Response::new(req.id, Some(sh));
        let json = resp.to_json()?;

        Ok(Some(json))
    }
}
