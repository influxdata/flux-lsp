use std::cell::RefCell;
use std::rc::Rc;

use flux::ast::walk::walk_rc;
use flux::ast::walk::{Node, Visitor};

use crate::protocol::properties::Position;
use crate::shared::ast::create_ast_package;
use crate::shared::conversion::flux_position_to_position;
use crate::shared::RequestContext;

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub position: Position,
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
        uri: String,
        ctx: RequestContext,
    ) -> Result<Option<PackageInfo>, String> {
        let package = create_ast_package(uri.clone(), ctx)?;
        let walker =
            Rc::new(flux::ast::walk::Node::File(&package.files[0]));
        let visitor = PackageFinderVisitor::default();

        walk_rc(&visitor, walker);

        let state = visitor.state.borrow();
        Ok(state.info.clone())
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
