use std::collections::hash_map::HashMap;
use std::sync::{Arc, Mutex};

fn get_dir(uri: &'_ str) -> String {
    let mut parts = uri.split('/').collect::<Vec<&str>>();
    parts.pop();
    parts.join("/")
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_get_dir() {
        let uri = "file:///users/test/mine.flux";
        let dir = get_dir(uri);

        assert_eq!(
            dir, "file:///users/test",
            "returns correct directory"
        )
    }
}

#[derive(Clone)]
pub struct CacheValue {
    pub uri: String,
    pub version: u32,
    pub contents: String,
}

#[derive(Default)]
pub struct Cache {
    store: Arc<Mutex<HashMap<String, CacheValue>>>,
}

impl Cache {
    pub fn remove(&self, uri: &'_ str) -> Result<(), String> {
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

    pub fn clear(&self) -> Result<(), String> {
        let keys = self.keys()?;

        for key in keys {
            self.remove(key.as_str())?;
        }

        Ok(())
    }

    pub fn keys(&self) -> Result<Vec<String>, String> {
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

    pub fn get_package(
        &self,
        uri: &'_ str,
        multiple_files: bool,
    ) -> Result<Vec<CacheValue>, String> {
        if !multiple_files {
            let result = self.get(uri)?;
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

    pub fn force(
        &self,
        uri: &'_ str,
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

        let val = CacheValue {
            uri: uri.to_string(),
            version,
            contents,
        };

        store.insert(uri.to_string(), val);

        Ok(())
    }

    pub fn set(
        &self,
        uri: &'_ str,
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

        if let Some(val) = store.get(uri) {
            if val.version <= version {
                let val = CacheValue {
                    uri: uri.to_string(),
                    version,
                    contents,
                };

                store.insert(uri.to_string(), val);
            }
        } else {
            let val = CacheValue {
                uri: uri.to_string(),
                version,
                contents,
            };

            store.insert(uri.to_string(), val);
        }

        Ok(())
    }

    pub fn get(&self, uri: &'_ str) -> Result<CacheValue, String> {
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
