use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::walk::walk_rc;
use flux::ast::walk::{Node, Visitor};

use crate::cache::Cache;
use crate::shared::ast::create_ast_package;
use crate::shared::conversion::flux_position_to_position;
use crate::shared::RequestContext;

use lsp_types as lsp;

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub position: lsp::Position,
}

#[derive(Default)]
pub struct PackageFinderState {
    pub info: Option<PackageInfo>,
}

#[derive(Clone, Default)]
pub struct PackageFinderVisitor {
    pub state: Rc<RefCell<PackageFinderState>>,
}

impl PackageFinderVisitor {
    pub fn find(
        uri: lsp::Url,
        ctx: RequestContext,
        cache: &Cache,
    ) -> Result<Option<PackageInfo>, String> {
        let package = create_ast_package(uri, ctx, cache)?;
        for file in package.files {
            let walker = Rc::new(flux::ast::walk::Node::File(&file));
            let visitor = PackageFinderVisitor::default();

            walk_rc(&visitor, walker);

            let state = visitor.state.borrow();
            if let Some(info) = state.info.clone() {
                return Ok(Some(info));
            }
        }

        Ok(None)
    }
}

impl<'a> Visitor<'a> for PackageFinderVisitor {
    fn visit(&self, node: Rc<Node<'a>>) -> Option<Self> {
        if let Node::PackageClause(p) = node.as_ref() {
            let mut state = self.state.borrow_mut();
            state.info = Some(PackageInfo {
                name: "".to_string(),
                position: flux_position_to_position(
                    p.base.location.clone().start,
                ),
            })
        }
        Some(self.clone())
    }
}
