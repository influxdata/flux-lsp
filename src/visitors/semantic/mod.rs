use flux::semantic::{
    nodes::{Expression, Symbol},
    walk::{self, Node, Visitor},
};
use lspower::lsp;

mod completion;
mod functions;
mod lint;
mod symbols;

pub use completion::{
    FunctionFinderVisitor, ObjectFunctionFinderVisitor,
};
pub use lint::{
    ContribDiagnosticVisitor, ExperimentalDiagnosticVisitor,
    InfluxDBIdentifierDiagnosticVisitor,
};
pub use symbols::SymbolsVisitor;

fn contains_position(node: Node<'_>, pos: lsp::Position) -> bool {
    if let Node::Package(_) = node {
        // flux::semantic::nodes::Package is walkable, but when multiple ast files are joined, Package appears to have
        // a start/end location of 0:0.
        return false;
    }
    let range: lsp::Range = node.loc().clone().into();

    if pos.line < range.start.line {
        return false;
    }

    if pos.line > range.end.line {
        return false;
    }

    if pos.line == range.start.line
        && pos.character < range.start.character
    {
        return false;
    }

    if pos.line == range.end.line
        && pos.character > range.end.character
    {
        return false;
    }

    true
}

#[derive(Debug)]
pub struct NodeFinderVisitor<'a> {
    pub node: Option<Node<'a>>,
    pub position: lsp::Position,
    pub path: Vec<Node<'a>>,
}

impl<'a> Visitor<'a> for NodeFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        let contains = contains_position(node, self.position);

        if contains {
            self.path.push(node);
            self.node = Some(node);
        }

        true
    }
}

impl<'a> NodeFinderVisitor<'a> {
    pub fn new(pos: lsp::Position) -> NodeFinderVisitor<'a> {
        NodeFinderVisitor {
            node: None,
            position: pos,
            path: vec![],
        }
    }
}

pub struct IdentFinderVisitor<'a> {
    pub name: Symbol,
    pub identifiers: Vec<walk::Node<'a>>,
}

impl<'a> Visitor<'a> for IdentFinderVisitor<'a> {
    fn visit(&mut self, node: walk::Node<'a>) -> bool {
        match node {
            walk::Node::MemberExpr(m) => {
                if let Expression::Identifier(i) = &m.object {
                    if i.name == self.name {
                        return true;
                    }
                }
                return false;
            }
            walk::Node::Identifier(n) => {
                if n.name == self.name {
                    self.identifiers.push(node);
                }
            }
            walk::Node::IdentifierExpr(n) => {
                if n.name == self.name {
                    self.identifiers.push(node);
                }
            }
            _ => {}
        }
        true
    }
}

impl<'a> IdentFinderVisitor<'a> {
    pub fn new(name: Symbol) -> IdentFinderVisitor<'a> {
        IdentFinderVisitor {
            name,
            identifiers: vec![],
        }
    }
}

pub struct DefinitionFinderVisitor<'a> {
    pub name: Symbol,
    pub node: Option<Node<'a>>,
}

impl<'a> Visitor<'a> for DefinitionFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        match node {
            walk::Node::VariableAssgn(v) => {
                if v.id.name == self.name {
                    self.node = Some(node);
                    return false;
                }

                true
            }
            walk::Node::BuiltinStmt(v) => {
                if v.id.name == self.name {
                    self.node = Some(walk::Node::Identifier(&v.id));
                    return false;
                }

                true
            }
            walk::Node::FunctionParameter(param) => {
                if param.key.name == self.name {
                    self.node = Some(node);
                    return false;
                }

                true
            }
            _ => true,
        }
    }
}

impl<'a> DefinitionFinderVisitor<'a> {
    pub fn new(name: Symbol) -> DefinitionFinderVisitor<'a> {
        DefinitionFinderVisitor { name, node: None }
    }
}

#[derive(Default)]
pub struct FoldFinderVisitor<'a> {
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Visitor<'a> for FoldFinderVisitor<'a> {
    fn visit(&mut self, node: Node<'a>) -> bool {
        if let Node::Block(_) = node {
            self.nodes.push(node);
        }

        true
    }
}

#[derive(Clone, Debug)]
pub struct Import {
    pub path: String,
    pub name: String,
}

#[derive(Default)]
pub struct ImportFinderVisitor {
    pub imports: Vec<Import>,
}

impl<'a> Visitor<'a> for ImportFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        if let Node::ImportDeclaration(import) = node {
            let name = match &import.alias {
                Some(alias) => alias.name.to_string(),
                None => {
                    // XXX: rockstar (15 Jul 2022) - This block duplicates effort found
                    // in `lang`.
                    import
                        .path
                        .value
                        .as_str()
                        .split('/')
                        .last()
                        .expect("Invalid package path/name supplied")
                        .to_string()
                }
            };

            self.imports.push(Import {
                path: import.path.value.clone(),
                name,
            });
        }

        true
    }
}

#[derive(Default)]
pub struct PackageNodeFinderVisitor {
    pub location: Option<lsp::Range>,
}

impl<'a> Visitor<'a> for PackageNodeFinderVisitor {
    fn visit(&mut self, node: Node<'a>) -> bool {
        if let Node::PackageClause(n) = node {
            self.location = Some(n.loc.clone().into());
            return false;
        }
        true
    }
}
