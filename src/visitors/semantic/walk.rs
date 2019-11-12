use std::rc::Rc;

use flux::ast::SourceLocation;
use flux::semantic::nodes::*;

#[derive(Debug)]
pub enum Node<'a> {
    Package(&'a Package),
    File(&'a File),
    PackageClause(&'a PackageClause),
    ImportDeclaration(&'a ImportDeclaration),
    Identifier(&'a Identifier),
    FunctionParameter(&'a FunctionParameter),
    Block(&'a Block),
    Property(&'a Property),

    // Expressions.
    IdentifierExpr(&'a IdentifierExpr),
    ArrayExpr(&'a ArrayExpr),
    FunctionExpr(&'a FunctionExpr),
    LogicalExpr(&'a LogicalExpr),
    ObjectExpr(&'a ObjectExpr),
    MemberExpr(&'a MemberExpr),
    IndexExpr(&'a IndexExpr),
    BinaryExpr(&'a BinaryExpr),
    UnaryExpr(&'a UnaryExpr),
    CallExpr(&'a CallExpr),
    ConditionalExpr(&'a ConditionalExpr),
    StringExpr(&'a StringExpr),
    IntegerLit(&'a IntegerLit),
    FloatLit(&'a FloatLit),
    StringLit(&'a StringLit),
    DurationLit(&'a DurationLit),
    UintLit(&'a UintLit),
    BooleanLit(&'a BooleanLit),
    DateTimeLit(&'a DateTimeLit),
    RegexpLit(&'a RegexpLit),

    // Statements.
    ExprStmt(&'a ExprStmt),
    OptionStmt(&'a OptionStmt),
    ReturnStmt(&'a ReturnStmt),
    TestStmt(&'a TestStmt),
    BuiltinStmt(&'a BuiltinStmt),

    // StringExprPart.
    TextPart(&'a TextPart),
    InterpolatedPart(&'a InterpolatedPart),

    // Assignment.
    VariableAssgn(&'a VariableAssgn),
    MemberAssgn(&'a MemberAssgn),
}

impl<'a> Node<'a> {
    pub fn loc(&self) -> &SourceLocation {
        match self {
            Node::Package(n) => &n.loc,
            Node::File(n) => &n.loc,
            Node::PackageClause(n) => &n.loc,
            Node::ImportDeclaration(n) => &n.loc,
            Node::Identifier(n) => &n.loc,
            Node::IdentifierExpr(n) => &n.loc,
            Node::ArrayExpr(n) => &n.loc,
            Node::FunctionExpr(n) => &n.loc,
            Node::FunctionParameter(n) => &n.loc,
            Node::LogicalExpr(n) => &n.loc,
            Node::ObjectExpr(n) => &n.loc,
            Node::MemberExpr(n) => &n.loc,
            Node::IndexExpr(n) => &n.loc,
            Node::BinaryExpr(n) => &n.loc,
            Node::UnaryExpr(n) => &n.loc,
            Node::CallExpr(n) => &n.loc,
            Node::ConditionalExpr(n) => &n.loc,
            Node::StringExpr(n) => &n.loc,
            Node::IntegerLit(n) => &n.loc,
            Node::FloatLit(n) => &n.loc,
            Node::StringLit(n) => &n.loc,
            Node::DurationLit(n) => &n.loc,
            Node::UintLit(n) => &n.loc,
            Node::BooleanLit(n) => &n.loc,
            Node::DateTimeLit(n) => &n.loc,
            Node::RegexpLit(n) => &n.loc,
            Node::ExprStmt(n) => &n.loc,
            Node::OptionStmt(n) => &n.loc,
            Node::ReturnStmt(n) => &n.loc,
            Node::TestStmt(n) => &n.loc,
            Node::BuiltinStmt(n) => &n.loc,
            Node::Block(n) => n.loc(),
            Node::Property(n) => &n.loc,
            Node::TextPart(n) => &n.loc,
            Node::InterpolatedPart(n) => &n.loc,
            Node::VariableAssgn(n) => &n.loc,
            Node::MemberAssgn(n) => &n.loc,
        }
    }
}

// Private utility functions for node conversion.
impl<'a> Node<'a> {
    fn from_expr(expr: &'a Expression) -> Node {
        match *expr {
            Expression::Identifier(ref e) => Node::IdentifierExpr(e),
            Expression::Array(ref e) => Node::ArrayExpr(e),
            Expression::Function(ref e) => Node::FunctionExpr(e),
            Expression::Logical(ref e) => Node::LogicalExpr(e),
            Expression::Object(ref e) => Node::ObjectExpr(e),
            Expression::Member(ref e) => Node::MemberExpr(e),
            Expression::Index(ref e) => Node::IndexExpr(e),
            Expression::Binary(ref e) => Node::BinaryExpr(e),
            Expression::Unary(ref e) => Node::UnaryExpr(e),
            Expression::Call(ref e) => Node::CallExpr(e),
            Expression::Conditional(ref e) => {
                Node::ConditionalExpr(e)
            }
            Expression::StringExpr(ref e) => Node::StringExpr(e),
            Expression::Integer(ref e) => Node::IntegerLit(e),
            Expression::Float(ref e) => Node::FloatLit(e),
            Expression::StringLit(ref e) => Node::StringLit(e),
            Expression::Duration(ref e) => Node::DurationLit(e),
            Expression::Uint(ref e) => Node::UintLit(e),
            Expression::Boolean(ref e) => Node::BooleanLit(e),
            Expression::DateTime(ref e) => Node::DateTimeLit(e),
            Expression::Regexp(ref e) => Node::RegexpLit(e),
        }
    }

    fn from_stmt(stmt: &'a Statement) -> Node {
        match *stmt {
            Statement::Expr(ref s) => Node::ExprStmt(s),
            Statement::Variable(ref s) => Node::VariableAssgn(s),
            Statement::Option(ref s) => Node::OptionStmt(s),
            Statement::Return(ref s) => Node::ReturnStmt(s),
            Statement::Test(ref s) => Node::TestStmt(s),
            Statement::Builtin(ref s) => Node::BuiltinStmt(s),
        }
    }

    fn from_string_expr_part(sp: &'a StringExprPart) -> Node {
        match *sp {
            StringExprPart::Text(ref t) => Node::TextPart(t),
            StringExprPart::Interpolated(ref e) => {
                Node::InterpolatedPart(e)
            }
        }
    }

    fn from_assignment(a: &'a Assignment) -> Node {
        match *a {
            Assignment::Variable(ref v) => Node::VariableAssgn(v),
            Assignment::Member(ref m) => Node::MemberAssgn(m),
        }
    }
}

pub trait Visitor<'a>: Sized {
    /// Visit is called for a node.
    /// When the Visitor is used in function `walk`, the boolean value returned
    /// is used to continue (true) or stop (false) walking.
    fn visit(&self, node: Rc<Node<'a>>) -> bool;
    /// Done is called for a node once it has been visited along with all of its children.
    /// The default is to do nothing
    fn done(&self, _: Rc<Node<'a>>) {}
}

/// Walk recursively visits children of a node given a Visitor.
/// Nodes are visited in depth-first order.
pub fn walk<'a, T>(v: &mut T, node: Rc<Node<'a>>)
where
    T: Visitor<'a>,
{
    if v.visit(node.clone()) {
        match *node.clone() {
            Node::Package(ref n) => {
                for file in n.files.iter() {
                    walk(v, Rc::new(Node::File(file)));
                }
            }
            Node::File(ref n) => {
                if let Some(ref pkg) = n.package {
                    walk(v, Rc::new(Node::PackageClause(pkg)));
                }
                for imp in n.imports.iter() {
                    walk(v, Rc::new(Node::ImportDeclaration(imp)));
                }
                for stmt in n.body.iter() {
                    walk(v, Rc::new(Node::from_stmt(stmt)));
                }
            }
            Node::PackageClause(ref n) => {
                walk(v, Rc::new(Node::Identifier(&n.name)));
            }
            Node::ImportDeclaration(ref n) => {
                if let Some(ref alias) = n.alias {
                    walk(v, Rc::new(Node::Identifier(alias)));
                }
                walk(v, Rc::new(Node::StringLit(&n.path)));
            }
            Node::Identifier(_) => {}
            Node::IdentifierExpr(_) => {}
            Node::ArrayExpr(ref n) => {
                for element in n.elements.iter() {
                    walk(v, Rc::new(Node::from_expr(element)));
                }
            }
            Node::FunctionExpr(ref n) => {
                for param in n.params.iter() {
                    walk(v, Rc::new(Node::FunctionParameter(param)));
                }
                walk(v, Rc::new(Node::Block(&n.body)));
            }
            Node::FunctionParameter(ref n) => {
                walk(v, Rc::new(Node::Identifier(&n.key)));
                if let Some(ref def) = n.default {
                    walk(v, Rc::new(Node::from_expr(def)));
                }
            }
            Node::LogicalExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.left)));
                walk(v, Rc::new(Node::from_expr(&n.right)));
            }
            Node::ObjectExpr(ref n) => {
                if let Some(ref i) = n.with {
                    walk(v, Rc::new(Node::IdentifierExpr(i)));
                }
                for prop in n.properties.iter() {
                    walk(v, Rc::new(Node::Property(prop)));
                }
            }
            Node::MemberExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.object)));
            }
            Node::IndexExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.array)));
                walk(v, Rc::new(Node::from_expr(&n.index)));
            }
            Node::BinaryExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.left)));
                walk(v, Rc::new(Node::from_expr(&n.right)));
            }
            Node::UnaryExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.argument)));
            }
            Node::CallExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.callee)));
                if let Some(ref p) = n.pipe {
                    walk(v, Rc::new(Node::from_expr(p)));
                }
                for arg in n.arguments.iter() {
                    walk(v, Rc::new(Node::Property(arg)));
                }
            }
            Node::ConditionalExpr(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.test)));
                walk(v, Rc::new(Node::from_expr(&n.consequent)));
                walk(v, Rc::new(Node::from_expr(&n.alternate)));
            }
            Node::StringExpr(ref n) => {
                for part in n.parts.iter() {
                    walk(
                        v,
                        Rc::new(Node::from_string_expr_part(part)),
                    );
                }
            }
            Node::IntegerLit(_) => {}
            Node::FloatLit(_) => {}
            Node::StringLit(_) => {}
            Node::DurationLit(_) => {}
            Node::UintLit(_) => {}
            Node::BooleanLit(_) => {}
            Node::DateTimeLit(_) => {}
            Node::RegexpLit(_) => {}
            Node::ExprStmt(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.expression)));
            }
            Node::OptionStmt(ref n) => {
                walk(
                    v,
                    Rc::new(Node::from_assignment(&n.assignment)),
                );
            }
            Node::ReturnStmt(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.argument)));
            }
            Node::TestStmt(ref n) => {
                walk(v, Rc::new(Node::VariableAssgn(&n.assignment)));
            }
            Node::BuiltinStmt(ref n) => {
                walk(v, Rc::new(Node::Identifier(&n.id)));
            }
            Node::Block(ref n) => match n {
                Block::Variable(ref assgn, ref next) => {
                    walk(v, Rc::new(Node::VariableAssgn(assgn)));
                    walk(v, Rc::new(Node::Block(&*next)));
                }
                Block::Expr(ref estmt, ref next) => {
                    walk(v, Rc::new(Node::ExprStmt(estmt)));
                    walk(v, Rc::new(Node::Block(&*next)))
                }
                Block::Return(ref expr) => {
                    walk(v, Rc::new(Node::from_expr(expr)))
                }
            },
            Node::Property(ref n) => {
                walk(v, Rc::new(Node::Identifier(&n.key)));
                walk(v, Rc::new(Node::from_expr(&n.value)));
            }
            Node::TextPart(_) => {}
            Node::InterpolatedPart(ref n) => {
                walk(v, Rc::new(Node::from_expr(&n.expression)));
            }
            Node::VariableAssgn(ref n) => {
                walk(v, Rc::new(Node::Identifier(&n.id)));
                walk(v, Rc::new(Node::from_expr(&n.init)));
            }
            Node::MemberAssgn(ref n) => {
                walk(v, Rc::new(Node::MemberExpr(&n.member)));
                walk(v, Rc::new(Node::from_expr(&n.init)));
            }
        };
    }
    v.done(node.clone());
}
