use std::collections::hash_map::HashMap;

use crate::protocol::properties::ContentChange;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref GLOBAL_CACHE: Cache = Cache::default();
}

pub fn set(
    uri: String,
    version: u32,
    contents: String,
) -> Result<(), String> {
    GLOBAL_CACHE.set(uri, version, contents)
}

pub fn get(uri: String) -> Result<CacheValue, String> {
    GLOBAL_CACHE.get(uri)
}

pub fn remove(uri: String) -> Result<(), String> {
    GLOBAL_CACHE.remove(uri)
}

pub fn apply(
    uri: String,
    version: u32,
    changes: Vec<ContentChange>,
) -> Result<(), String> {
    for change in changes {
        if change.range.is_none() {
            return set(uri, version, change.text);
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct CacheValue {
    pub uri: String,
    pub version: u32,
    pub contents: String,
}

#[derive(Default)]
struct Cache {
    store: Arc<Mutex<HashMap<String, CacheValue>>>,
}

impl Cache {
    fn remove(&self, uri: String) -> Result<(), String> {
        let mut store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        store.remove(&uri);

        Ok(())
    }

    fn set(
        &self,
        uri: String,
        version: u32,
        contents: String,
    ) -> Result<(), String> {
        let mut store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        if let Some(val) = store.get(&uri) {
            if val.version <= version {
                let val = CacheValue {
                    uri: uri.clone(),
                    version,
                    contents,
                };

                store.insert(uri, val);
            }
        } else {
            let val = CacheValue {
                uri: uri.clone(),
                version,
                contents,
            };

            store.insert(uri, val);
        }

        Ok(())
    }

    fn get(&self, uri: String) -> Result<CacheValue, String> {
        let store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        if let Some(cv) = store.get(&uri) {
            Ok((*cv).clone())
        } else {
            Err(format!("unknown uri: {}", uri.clone()))
        }
    }
}
