mod results;
mod utils;

use results::*;
use utils::*;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::protocol::properties::Position;
use crate::shared::signatures::get_argument_names;
use crate::shared::Function;
use crate::stdlib::Completable;

use flux::semantic::nodes::*;
use flux::semantic::types::MonoType;
use flux::semantic::walk::{Node, Visitor};

pub struct FunctionFinderState {
    pub functions: Vec<Function>,
}

pub struct FunctionFinderVisitor {
    pub pos: Position,
    pub state: Arc<Mutex<FunctionFinderState>>,
}

impl FunctionFinderVisitor {
    pub fn new(pos: Position) -> Self {
        FunctionFinderVisitor {
            pos,
            state: Arc::new(Mutex::new(FunctionFinderState {
                functions: vec![],
            })),
        }
    }
}

impl<'a> Visitor<'a> for FunctionFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let loc = node.loc();
            let pos = self.pos.clone();

            if defined_after(loc, pos) {
                return true;
            }

            if let Node::VariableAssgn(assgn) = node.as_ref() {
                let name = assgn.id.name.clone();

                if let Expression::Function(f) = assgn.init.clone() {
                    if let MonoType::Fun(fun) = f.typ.clone() {
                        let mut params = get_argument_names(fun.req);
                        for opt in get_argument_names(fun.opt) {
                            params.push(opt);
                        }

                        state
                            .functions
                            .push(Function { name, params })
                    }
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    assgn,
                ) = &opt.assignment
                {
                    let name = assgn.id.name.clone();
                    if let Expression::Function(f) =
                        assgn.init.clone()
                    {
                        if let MonoType::Fun(fun) = f.typ.clone() {
                            let mut params =
                                get_argument_names(fun.req);
                            for opt in get_argument_names(fun.opt) {
                                params.push(opt);
                            }

                            state
                                .functions
                                .push(Function { name, params })
                        }
                    }
                }
            }
        }

        true
    }
}

#[derive(Default)]
pub struct CompletableFinderState {
    pub completables: Vec<Arc<dyn Completable + Send + Sync>>,
}

pub struct CompletableFinderVisitor {
    pub pos: Position,
    pub state: Arc<Mutex<CompletableFinderState>>,
}

impl<'a> Visitor<'a> for CompletableFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let loc = node.loc();
            let pos = self.pos.clone();

            if defined_after(loc, pos) {
                return true;
            }

            if let Node::ImportDeclaration(id) = node.as_ref() {
                if let Some(alias) = id.alias.clone() {
                    (*state).completables.push(Arc::new(
                        ImportAliasResult::new(
                            id.path.value.clone(),
                            alias.name,
                        ),
                    ));
                }
            }

            if let Node::VariableAssgn(assgn) = node.as_ref() {
                let name = assgn.id.name.clone();
                if let Some(var_type) = get_var_type(&assgn.init) {
                    (*state).completables.push(Arc::new(VarResult {
                        var_type,
                        name: name.clone(),
                    }));
                }

                if let Some(fun) =
                    create_function_result(name, &assgn.init)
                {
                    (*state).completables.push(Arc::new(fun));
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    var_assign,
                ) = &opt.assignment
                {
                    let name = var_assign.id.name.clone();
                    if let Some(var_type) =
                        get_var_type(&var_assign.init)
                    {
                        (*state).completables.push(Arc::new(
                            VarResult { var_type, name },
                        ));

                        return false;
                    }

                    if let Some(fun) =
                        create_function_result(name, &var_assign.init)
                    {
                        (*state).completables.push(Arc::new(fun));
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl CompletableFinderVisitor {
    pub fn new(pos: Position) -> Self {
        CompletableFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableFinderState::default(),
            )),
            pos,
        }
    }
}

#[derive(Clone)]
pub struct ObjectFunction {
    pub object: String,
    pub function: Function,
}

#[derive(Default)]
pub struct ObjectFunctionFinderState {
    pub results: Vec<ObjectFunction>,
}

#[derive(Default)]
pub struct ObjectFunctionFinderVisitor {
    pub state: Arc<Mutex<ObjectFunctionFinderState>>,
}

impl<'a> Visitor<'a> for ObjectFunctionFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        match node.as_ref() {
            Node::VariableAssgn(assignment) => {
                let object_name = assignment.id.name.clone();

                if let Expression::Object(obj) =
                    assignment.init.clone()
                {
                    for prop in obj.properties.clone() {
                        let func_name = prop.key.name;

                        if let Expression::Function(fun) = prop.value
                        {
                            let params = fun
                                .params
                                .into_iter()
                                .map(|p| p.key.name)
                                .collect::<Vec<String>>();

                            if let Ok(mut state) = self.state.lock() {
                                state.results.push(ObjectFunction {
                                    object: object_name,
                                    function: Function {
                                        name: func_name,
                                        params,
                                    },
                                });

                                return false;
                            }
                        }
                    }
                }
            }
            Node::OptionStmt(opt) => {
                if let flux::semantic::nodes::Assignment::Variable(
                    assignment,
                ) = opt.assignment.clone()
                {
                    let object_name = assignment.id.name;
                    if let Expression::Object(obj) = assignment.init {
                        for prop in obj.properties.clone() {
                            let func_name = prop.key.name;

                            if let Expression::Function(fun) =
                                prop.value
                            {
                                let params = fun
                                    .params
                                    .into_iter()
                                    .map(|p| p.key.name)
                                    .collect::<Vec<String>>();

                                if let Ok(mut state) =
                                    self.state.lock()
                                {
                                    state.results.push(
                                        ObjectFunction {
                                            object: object_name,
                                            function: Function {
                                                name: func_name,
                                                params,
                                            },
                                        },
                                    );

                                    return false;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        true
    }
}

#[derive(Default)]
pub struct CompletableObjectFinderState {
    pub completables: Vec<Arc<dyn Completable + Send + Sync>>,
}

pub struct CompletableObjectFinderVisitor {
    pub name: String,
    pub state: Arc<Mutex<CompletableObjectFinderState>>,
}

impl CompletableObjectFinderVisitor {
    pub fn new(name: String) -> Self {
        CompletableObjectFinderVisitor {
            state: Arc::new(Mutex::new(
                CompletableObjectFinderState::default(),
            )),
            name,
        }
    }
}

impl<'a> Visitor<'a> for CompletableObjectFinderVisitor {
    fn visit(&mut self, node: Rc<Node<'a>>) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let name = self.name.clone();

            if let Node::ObjectExpr(obj) = node.as_ref() {
                if let Some(ident) = &obj.with {
                    if name == ident.name {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;
                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    VarResult {
                                        var_type,
                                        name: name.clone(),
                                    },
                                ));
                            }
                            if let Some(fun) = create_function_result(
                                name,
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }
                    }
                }
            }

            if let Node::VariableAssgn(assign) = node.as_ref() {
                if assign.id.name == name {
                    if let Expression::Object(obj) = &assign.init {
                        for prop in obj.properties.clone() {
                            let name = prop.key.name;

                            if let Some(var_type) =
                                get_var_type(&prop.value)
                            {
                                (*state).completables.push(Arc::new(
                                    VarResult {
                                        var_type,
                                        name: name.clone(),
                                    },
                                ));
                            }

                            if let Some(fun) = create_function_result(
                                name,
                                &prop.value,
                            ) {
                                (*state)
                                    .completables
                                    .push(Arc::new(fun));
                            }
                        }

                        return false;
                    }
                }
            }

            if let Node::OptionStmt(opt) = node.as_ref() {
                if let flux::semantic::nodes::Assignment::Variable(
                    assign,
                ) = opt.assignment.clone()
                {
                    if assign.id.name == name {
                        if let Expression::Object(obj) = assign.init {
                            for prop in obj.properties.clone() {
                                let name = prop.key.name;
                                if let Some(var_type) =
                                    get_var_type(&prop.value)
                                {
                                    (*state).completables.push(
                                        Arc::new(VarResult {
                                            var_type,
                                            name: name.clone(),
                                        }),
                                    );
                                }
                                if let Some(fun) =
                                    create_function_result(
                                        name,
                                        &prop.value,
                                    )
                                {
                                    (*state)
                                        .completables
                                        .push(Arc::new(fun));
                                }
                            }
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}
