#![allow(dead_code)]

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use lspower::lsp;

use super::types::LspError;

// Url is parsed and validated prior to this function. `unwrap` here
// is okay.
#[allow(clippy::unwrap_used)]
fn url_to_key_val(url: &lsp::Url) -> (String, String, String) {
    let path = Path::new(url.path());

    let parent: String = path.parent().unwrap().display().to_string();
    let filename = path.file_name().unwrap().to_str().unwrap();

    (url.scheme().to_owned(), parent, filename.into())
}

fn get_analyzer() -> Result<
    flux::semantic::Analyzer<
        'static,
        &'static flux::semantic::import::Packages,
    >,
    LspError,
> {
    match flux::new_semantic_analyzer(
        flux::semantic::AnalyzerConfig::default(),
    ) {
        Ok(analyzer) => Ok(analyzer),
        Err(err) => {
            return Err(LspError::InternalError(format!("{}", err)))
        }
    }
}

/// Store acts as the in-memory storage backend for the LSP server.
///
/// The spec talks specifically about setting versions for files, but isn't
/// clear on how those versions are surfaced to the client, if ever. This
/// type could be extended to keep track of versions of files, but simplicity
/// is preferred at this point.
pub(crate) struct Store {
    backend: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            backend: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Store {
    pub fn put(&self, url: &lsp::Url, contents: &str) {
        let (_, key, val) = url_to_key_val(url);

        match self.backend.write() {
            Ok(mut store) => match store.entry(key) {
                Entry::Vacant(entry) => {
                    let mut map = HashMap::new();
                    map.insert(val, contents.into());
                    entry.insert(map);
                }
                Entry::Occupied(mut entry) => {
                    let map = entry.get_mut();
                    map.insert(val, contents.into());
                }
            },
            Err(error) => {
                log::error!(
                    "Could not acquire store lock. Error: {}",
                    error
                );
            }
        }
    }

    pub fn remove(&self, url: &lsp::Url) {
        let (_, key, val) = url_to_key_val(url);

        match self.backend.write() {
            Ok(mut store) => match store.entry(key) {
                Entry::Vacant(_) => {
                    log::warn!(
                        "remove called on non-existent file: {}",
                        url
                    )
                }
                Entry::Occupied(mut entry) => {
                    let map = entry.get_mut();
                    map.remove(&val);
                }
            },
            Err(error) => {
                log::error!(
                    "Could not acquire store lock. Error: {}",
                    error
                );
            }
        }
    }

    pub fn get(&self, url: &lsp::Url) -> Result<String, LspError> {
        let (_, key, val) = url_to_key_val(url);

        match self.backend.read() {
            Ok(store) => match store.get(&key) {
                None => Err(LspError::FileNotFound(url.to_string())),
                Some(entry) => match entry.get(&val) {
                    Some(value) => Ok(value.into()),
                    None => {
                        Err(LspError::FileNotFound(url.to_string()))
                    }
                },
            },
            Err(_) => Err(LspError::LockNotAcquired),
        }
    }

    /// Get urls for all files in a specified file's package.
    pub fn get_package_urls(&self, url: &lsp::Url) -> Vec<lsp::Url> {
        let (scheme, key, _) = url_to_key_val(url);
        match self.backend.read() {
            Ok(store) => match store.get(&key) {
                None => vec![],
                Some(files) => files
                    .keys()
                    .map(|file| {
                        #[allow(clippy::unwrap_used)]
                        lsp::Url::parse(&format!(
                            "{}://{}/{}",
                            scheme, key, file
                        ))
                        .unwrap()
                    })
                    .collect(),
            },
            Err(_) => vec![],
        }
    }

    fn get_files(
        &self,
        path: String,
    ) -> Result<Vec<(String, String)>, LspError> {
        match self.backend.read() {
            Ok(store) => match store.get(&path) {
                None => Err(LspError::FileNotFound(path)),
                Some(entry) => {
                    Ok(entry
                        .keys()
                        .map(|key| {
                            (
                                key.clone(),
                                // Unwrap is okay here, as the key is retrieved from
                                // map.keys()
                                #[allow(clippy::unwrap_used)]
                                entry.get(key).unwrap().clone(),
                            )
                        })
                        .collect())
                }
            },
            Err(_) => Err(LspError::LockNotAcquired),
        }
    }

    pub fn get_ast_file(
        &self,
        url: &lsp::Url,
    ) -> Result<flux::ast::File, LspError> {
        let (_, _key, filename) = url_to_key_val(url);
        let source = match self.get(url) {
            Ok(value) => value,
            Err(_err) => {
                return Err(LspError::FileNotFound(filename))
            }
        };

        let file: flux::ast::File =
            flux::parser::parse_string(filename, &source);
        Ok(file)
    }

    fn get_ast_package(
        &self,
        url: &lsp::Url,
    ) -> Result<flux::ast::Package, LspError> {
        let (_, key, val) = url_to_key_val(url);
        let files = self.get_files(key)?;

        // Grab the AST Package corresponding to currently requested package. Merge all
        // other packages with it that one as root.
        let mut pkgs: Vec<flux::ast::Package> = files
            .iter()
            .map(|source| {
                flux::parser::parse_string(
                    source.0.clone(),
                    &source.1,
                )
                .into()
            })
            .collect();
        let mut ast_pkg = match pkgs
            .iter()
            .position(|pkg| pkg.files[0].name == val)
        {
            Some(idx) => pkgs.remove(idx),
            None => unreachable!(
                "File requested was not in list of packages returned"
            ),
        };

        for mut pkg in pkgs.into_iter() {
            if let Err(_error) =
                flux::merge_packages(&mut ast_pkg, &mut pkg)
            {
                // XXX: rockstar (3 Mar 2020) - Currently, this will discard any files that don't
                // match the source file's package clause. This should really happen at a check state
                // later, but this is how it works for now.
                continue;
            }
        }
        // XXX: rockstar (7 Mar 2022) - An ordering of these files has to be deterministic, but
        // flux itself hasn't really established a mechanism whereby these packages _should_ be ordered.
        // This hack allows us to make the files ordered deterministically so that the user can at least
        // understand what's happening, but this is not a permanent fix.
        // See: https://github.com/influxdata/flux/issues/4538
        ast_pkg.files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ast_pkg)
    }

    pub fn get_semantic_package(
        &self,

        url: &lsp::Url,
    ) -> Result<flux::semantic::nodes::Package, LspError> {
        let ast_pkg = self.get_ast_package(url)?;

        let mut analyzer = get_analyzer()?;
        match analyzer.analyze_ast(&ast_pkg) {
            Ok((_, pkg)) => Ok(pkg),
            Err(e) => {
                let error_string = format!("{}", e);
                if e.value.is_none() {
                    log::debug!("Unable to parse source: {}", e);
                }
                match e.value.map(|(_, sem_pkg)| sem_pkg) {
                    Some(value) => Ok(value),
                    None => {
                        Err(LspError::InternalError(error_string))
                    }
                }
            }
        }
    }

    pub fn get_package_errors(
        &self,
        url: &lsp::Url,
    ) -> Option<flux::semantic::FileErrors> {
        let ast_pkg = match self.get_ast_package(url) {
            Ok(pkg) => pkg,
            Err(err) => {
                log::error!("{:?}", err);
                return None;
            }
        };

        let mut analyzer = match get_analyzer() {
            Ok(analyzer) => analyzer,
            Err(err) => {
                log::error!("{:?}", err);
                return None;
            }
        };
        match analyzer.analyze_ast(&ast_pkg) {
            Ok(_) => None,
            Err(errors) => Some(errors.error),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
mod test {
    use super::*;

    #[test]
    fn put() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";
        store.put(&url, contents);

        let (_, key, val) = url_to_key_val(&url);

        {
            let mut backend = store
                .backend
                .write()
                .expect("Could not acquire lock");
            match backend.entry(key.clone()) {
                Entry::Vacant(_) => panic!("put to {} failed", key),
                Entry::Occupied(entry) => {
                    let map = entry.get();
                    match map.get(&val) {
                        Some(value) => assert_eq!(value, contents),
                        None => panic!("put to {} failed", key),
                    }
                }
            }
        }
    }

    #[test]
    fn get() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";
        let (_, key, val) = url_to_key_val(&url);

        {
            let mut map = HashMap::new();
            map.insert(val, contents.into());
            let mut backend = store
                .backend
                .write()
                .expect("Could not acquire lock");
            backend.insert(key, map);
        }

        let result = store.get(&url);

        assert_eq!(
            contents,
            result.expect("result is unexpectedly None")
        )
    }

    #[test]
    fn get_package_urls_single_file() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        store.put(&url, "");

        let urls = store.get_package_urls(&url);

        assert_eq!(vec![url,], urls);
    }

    #[test]
    fn get_package_urls_twe_files_two_packages() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        store.put(&url, "");
        store.put(&lsp::Url::parse("file:///a/c/c").unwrap(), "");

        let urls = store.get_package_urls(&url);

        assert_eq!(vec![url,], urls);
    }

    #[test]
    fn get_package_urls_two_files_one_package() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        let url2 = lsp::Url::parse("file:///a/b/d").unwrap();
        store.put(&url, "");
        store.put(&url2, "");

        let mut urls = store.get_package_urls(&url);
        urls.sort();

        assert_eq!(vec![url, url2], urls);
    }

    #[test]
    fn get_package_urls_three_files_two_packages() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        let url2 = lsp::Url::parse("file:///a/b/d").unwrap();
        store.put(&url, "");
        store.put(&url2, "");
        store.put(&lsp::Url::parse("file:///a/c/c").unwrap(), "");

        let mut urls = store.get_package_urls(&url);
        urls.sort();

        assert_eq!(vec![url, url2], urls);
    }

    #[test]
    fn remove() {
        let store = Store::default();
        let url = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";
        let (_, key, val) = url_to_key_val(&url);

        {
            let mut map = HashMap::new();
            map.insert(val, contents.into());
            let mut backend = store
                .backend
                .write()
                .expect("Could not acquire lock");
            backend.insert(key, map);
        }

        store.remove(&url);
        let result = store.get(&url);

        assert!(result.is_err());
    }

    #[test]
    fn get_semantic_package() {
        let store = Store::default();
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = r#"import "foo"

from(bucket: "bucket")
|> range(start: -15m)
|> filter(fn: (r) => r.tag == "anTag")"#;
        store.put(&key, contents);

        let result = store.get_semantic_package(&key);

        assert!(result.is_ok());
    }

    #[test]
    fn get_package_multi_file() {
        let store = Store::default();

        store.put(
            &lsp::Url::parse("file:///a/b/a").unwrap(),
            r#"v = {a: "b"}"#,
        );
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = r#"import "foo"

from(bucket: "bucket")
|> range(start: -15m)
|> filter(fn: (r) => r.tag == "anTag")"#;
        store.put(&key, contents);

        let result = store.get_semantic_package(&key).unwrap();

        assert_eq!(2, result.files.len());
    }

    #[test]
    fn get_package_multi_file_separate_packages() {
        let store = Store::default();

        store.put(
            &lsp::Url::parse("file:///a/b/a").unwrap(),
            r#"package b
v = {a: "b"}"#,
        );
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = r#"import "foo"

from(bucket: "bucket")
|> range(start: -15m)
|> filter(fn: (r) => r.tag == "anTag")"#;
        store.put(&key, contents);

        let result = store.get_semantic_package(&key).unwrap();

        assert_eq!(1, result.files.len());
    }

    #[test]
    fn get_package_errors_no_errors() {
        let store = Store::default();
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = r#"from(bucket: "bucket")
|> range(start: -15m)
|> filter(fn: (r) => r.tag == "anTag")"#;
        store.put(&key, contents);

        let result = store.get_package_errors(&key);

        assert!(result.is_none());
    }

    #[test]
    fn get_package_errors() {
        let store = Store::default();
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = r#"import "foo"

from(bucket: "bucket")
|> range(start: -15m)
|> filter(fn: (r) => r.tag == "anTag")"#;
        store.put(&key, contents);

        let result = store.get_package_errors(&key);

        // XXX: rockstar (29 Apr 2022) - fluxcore::errors is private, so asserting
        // information _about_ the errors is difficult. Asserting that there _are_ errors
        // is enough, for now.
        assert!(result.is_some());
    }
}
