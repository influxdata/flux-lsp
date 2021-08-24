use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::walk::{Node, Visitor};

use crate::shared::conversion::flux_position_to_position;

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
