#![allow(dead_code)]

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lspower::lsp;

/// LSPStore acts as the in-memory storage backend for the LSP server.
/// 
/// The spec talks specifically about setting versions for files, but isn't
/// clear on how those versions are surfaced to the client, if ever. This
/// type could be extended to keep track of versions of files, but simplicity
/// is preferred at this point.
pub(crate) struct Store {
    backend: Arc<Mutex<HashMap<lsp::Url, String>>>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            backend: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Store {
    pub fn put(
        &self,
        key: &lsp::Url,
        contents: &str,
    ) {
        match self.backend.lock() {
            Ok(mut store) => {
                match store.entry(key.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(contents.into());
                    }
                    Entry::Occupied(mut entry) => {
                        // The protocol spec is unclear on whether trying to open a file
                        // that is already opened is allowed, and research would indicate that
                        // there are badly behaved clients that do this. Rather than making this
                        // error, log the issue and move on.
                        log::warn!(
                        "Overwriting contents of existing key: {}",
                        entry.key(),
                    );
                        entry.insert(contents.into());
                    }
                }
            }
            Err(error) => {
                log::error!(
                    "Could not acquire store lock. Error: {}",
                    error
                );
            }
        }
    }

    pub fn remove(
        &self,
        key: &lsp::Url,
    ) {
        match self.backend.lock() {
            Ok(mut store) => {
                if store.remove(&key).is_none() {
                    // The protocol spec is unclear on whether trying to close a file
                    // that isn't open is allowed. To stop consistent with the
                    // implementation of textDocument/didOpen, this error is logged and
                    // allowed.
                    log::warn!(
                        "Cannot remove non-existent key: {}",
                        key
                    );
                }
            },
            Err(error) => {
                log::error!(
                    "Could not acquire store lock. Error: {}", error
                )
            }
        }
    }

    pub fn get(&self, key: &lsp::Url) -> Option<String> {
        match self.backend.lock() {
            Ok(mut store) => match store.entry(key.clone()) {
                Entry::Vacant(_) => None,
                Entry::Occupied(entry) => Some(entry.get().into()),
            },
            Err(error) => {
                log::error!(
                    "Could not acquire store lock. Error: {}",
                    error
                );
                None
            }
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
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";
        store.put(&key, contents);

        match store.backend.lock() {
            Ok(mut backend) => match backend.entry(key.clone()) {
                Entry::Vacant(_) => {
                    panic!("put to {} failed", key)
                }
                Entry::Occupied(entry) => {
                    assert_eq!(entry.get(), contents)
                }
            },
            Err(error) => panic!("Could not acquire lock: {}", error),
        };
    }

    #[test]
    fn get() {
        let store = Store::default();
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";

        {
            let mut backend =
                store.backend.lock().expect("Could not acquire lock");
            if let Entry::Vacant(entry) =
                backend.entry(key.clone())
            {
                entry.insert(contents.into());
            }
        }

        let result = store.get(&key);

        assert_eq!(
            contents,
            result.expect("result is unexpectedly None")
        )
    }

    #[test]
    fn remove() {
        let store = Store::default();
        let key = lsp::Url::parse("file:///a/b/c").unwrap();
        let contents = "import \"foo\"";

        {
            let mut backend =
                store.backend.lock().expect("Could not acquire lock");
            if let Entry::Vacant(entry) =
                backend.entry(key.clone())
            {
                entry.insert(contents.into());
            }
        }

        store.remove(&key);
        let result = store.get(&key);

        assert!(result.is_none());
    }
}
