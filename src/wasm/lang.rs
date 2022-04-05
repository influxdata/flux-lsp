use wasm_bindgen::prelude::*;

/// Flux provides an API for transforming, formatting, and checking syntax of flux source code.
#[wasm_bindgen]
#[derive(Debug, PartialEq)]
pub struct Flux {
    ast: flux::ast::Package,
}

#[wasm_bindgen]
impl Flux {
    /// Create a new Flux object from a raw flux string.
    #[wasm_bindgen(constructor)]
    pub fn new(script: &str) -> Self {
        let mut parser = flux::parser::Parser::new(script);
        let parsed = parser.parse_file("".into());

        Self { ast: parsed.into() }
    }

    #[wasm_bindgen]
    pub fn from_ast(obj: JsValue) -> Self {
        match obj.into_serde::<flux::ast::Package>() {
            Ok(ast) => Self { ast },
            Err(e) => {
                log::error!("{}", e);
                let mut parser = flux::parser::Parser::new("");
                let parsed = parser.parse_file("".into());

                Self { ast: parsed.into() }
            }
        }
    }

    /// Get the ast from a Flux instance
    pub fn ast(&self) -> JsValue {
        match JsValue::from_serde(&self.ast) {
            Ok(value) => value,
            Err(err) => {
                log::error!("{}", err);
                JsValue::NULL
            }
        }
    }

    /// Format the flux.
    ///
    /// In the event that the flux is invalid syntax, an Err will be returned,
    /// which will translate into a JavaScript exception being thrown.
    #[wasm_bindgen]
    pub fn format(&self) -> Result<String, String> {
        // XXX: rockstar (1 Apr 2022) - This currently only supports a single file package. It should
        // take a parameter for the file to format.
        flux::formatter::convert_to_string(&self.ast.files[0])
            .map_err(|err| format!("{}", err))
    }

    /// Check that the flux is valid.
    ///
    /// This function does a semantic check, which will check types and builtin
    /// function signatures, which can't be checked via a base AST check.
    #[wasm_bindgen]
    pub fn is_valid(&self) -> bool {
        let mut analyzer = match flux::new_semantic_analyzer(
            flux::semantic::AnalyzerConfig {
                features: vec![],
            },
        ) {
            Ok(analyzer) => analyzer,
            Err(_) => return false,
        };
        match analyzer.analyze_ast(&self.ast) {
            Ok((_, _pkg)) => true,
            Err(_) => false,
        }
    }
}
