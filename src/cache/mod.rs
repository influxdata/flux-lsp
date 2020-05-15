use std::collections::hash_map::HashMap;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref GLOBAL_CACHE: Cache = Cache::default();
}

fn get_dir(uri: String) -> String {
    let mut parts = uri.split('/').collect::<Vec<&str>>();
    parts.pop();
    parts.join("/")
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_get_dir() {
        let uri = String::from("file:///users/test/mine.flux");
        let dir = get_dir(uri);

        assert_eq!(
            dir, "file:///users/test",
            "returns correct directory"
        )
    }
}

pub fn set(
    uri: String,
    version: u32,
    contents: String,
) -> Result<(), String> {
    GLOBAL_CACHE.set(uri, version, contents)
}

pub fn get(uri: String) -> Result<CacheValue, String> {
    GLOBAL_CACHE.get(uri.as_str())
}

pub fn get_package(
    uri: String,
    multiple_files: bool,
) -> Result<Vec<CacheValue>, String> {
    GLOBAL_CACHE.get_package(uri, multiple_files)
}

pub fn remove(uri: String) -> Result<(), String> {
    GLOBAL_CACHE.remove(uri.as_str())
}

pub fn clear() -> Result<(), String> {
    GLOBAL_CACHE.clear()
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
    fn remove(&self, uri: &'_ str) -> Result<(), String> {
        let mut store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        store.remove(uri);

        Ok(())
    }

    #[allow(dead_code)]
    fn clear(&self) -> Result<(), String> {
        let keys = self.keys()?;

        for key in keys {
            self.remove(key.as_str())?;
        }

        Ok(())
    }

    fn keys(&self) -> Result<Vec<String>, String> {
        let store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        Ok(store.keys().map(|k| (*k).clone()).collect())
    }

    fn get_package(
        &self,
        uri: String,
        multiple_files: bool,
    ) -> Result<Vec<CacheValue>, String> {
        if !multiple_files {
            let result = self.get(uri.as_str())?;
            return Ok(vec![result]);
        }

        let dir = get_dir(uri);
        let keys = self.keys()?;

        Ok(keys
            .into_iter()
            .filter(|x: &String| x.starts_with(dir.as_str()))
            .fold(vec![], |mut acc, x| {
                if let Ok(v) = self.get(x.as_str()) {
                    acc.push(v);
                }

                acc
            }))
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

        if let Some(val) = store.get(uri.as_str()) {
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

    fn get(&self, uri: &'_ str) -> Result<CacheValue, String> {
        let store = match self.store.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(
                    "failed to get cache store lock".to_string()
                )
            }
        };

        if let Some(cv) = store.get(uri) {
            Ok((*cv).clone())
        } else {
            Ok(CacheValue {
                uri: uri.to_string(),
                version: 1,
                contents: "".to_string(),
            })
        }
    }
}
