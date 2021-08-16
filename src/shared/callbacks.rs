use js_sys::{Function, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Default)]
pub struct ResolvableState {
    pub waker: Option<Waker>,
    pub result: Vec<String>,
}

#[derive(Clone)]
pub struct Resolvable {
    pub resolved: Arc<Mutex<bool>>,
    pub state: Arc<Mutex<ResolvableState>>,
}

#[allow(clippy::mutex_atomic)]
impl Default for Resolvable {
    fn default() -> Self {
        Resolvable {
            resolved: Arc::new(Mutex::new(false)),
            state: Arc::new(Mutex::new(ResolvableState::default())),
        }
    }
}

impl Resolvable {
    pub fn resolve(&mut self, val: Vec<String>) {
        if let Ok(mut resolved) = self.resolved.lock() {
            *resolved = true;
        }

        if let Ok(mut state) = self.state.lock() {
            if let Some(waker) = state.waker.clone() {
                waker.wake();
            }

            state.result = val;
        }
    }

    pub fn result(&mut self) -> Vec<String> {
        if let Ok(state) = self.state.lock() {
            return state.result.clone();
        }

        vec![]
    }

    pub fn is_resolved(&self) -> bool {
        if let Ok(resolved) = self.resolved.lock() {
            return *resolved;
        }

        false
    }
}

impl Future for Resolvable {
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        ctx: &mut Context,
    ) -> Poll<Self::Output> {
        if self.is_resolved() {
            Poll::Ready(())
        } else {
            let waker = ctx.waker().clone();
            if let Ok(mut state) = self.state.lock() {
                state.waker = Some(waker);
            }
            Poll::Pending
        }
    }
}

#[derive(Clone)]
pub struct Callback {
    f: Function,
}

#[allow(dead_code)]
impl Callback {
    pub fn new(f: Function) -> Self {
        Callback { f }
    }

    pub fn call0(&self) -> Result<Promise, String> {
        match self.f.call0(&JsValue::NULL) {
            Ok(result) => Ok(Promise::from(result)),
            Err(_e) => Err("Callback failed".to_string()),
        }
    }

    pub fn call1(&self, arg1: JsValue) -> Result<Promise, String> {
        match self.f.call1(&JsValue::NULL, &arg1) {
            Ok(result) => Ok(Promise::from(result)),
            Err(_e) => Err("Callback failed".to_string()),
        }
    }

    pub fn call2(
        &self,
        arg1: JsValue,
        arg2: JsValue,
    ) -> Result<Promise, String> {
        match self.f.call2(&JsValue::NULL, &arg1, &arg2) {
            Ok(result) => Ok(Promise::from(result)),
            Err(_e) => Err("Callback failed".to_string()),
        }
    }
}

unsafe impl<'a> Send for Callback {}
unsafe impl<'a> Sync for Callback {}

#[derive(Default, Clone)]
pub struct Callbacks {
    pub buckets: Option<Callback>,
    pub measurements: Option<Callback>,
    pub tag_keys: Option<Callback>,
    pub tag_values: Option<Callback>,
}

#[allow(dead_code)]
impl Callbacks {
    // All of the `register_*_callback` methods have been disabled
    // to prevent the LSP from crashing. Autocomplete for user-specific
    // data will not be available until these methods are re-enabled
    // See the following issue for more details:
    // https://github.com/influxdata/flux-lsp/issues/190
    pub fn register_buckets_callback(&mut self, _f: Function) {
        self.buckets = None;
        // self.buckets = Some(Callback::new(f)) // re-enable buckets callback
    }

    pub fn register_measurements_callback(&mut self, _f: Function) {
        self.measurements = None;
    }

    pub fn register_tag_keys_callback(&mut self, _f: Function) {
        self.tag_keys = None;
    }

    pub fn register_tag_values_callback(&mut self, _f: Function) {
        self.tag_values = None;
    }

    fn call_buckets(&self) -> Result<JsFuture, String> {
        if let Some(cb) = self.buckets.clone() {
            let promise = cb.call0()?;
            Ok(JsFuture::from(promise))
        } else {
            Err("No buckets function set".to_string())
        }
    }

    fn call_measurements(
        &self,
        bucket: String,
    ) -> Result<JsFuture, String> {
        if let Some(cb) = self.measurements.clone() {
            let promise = cb.call1(bucket.into())?;
            Ok(JsFuture::from(promise))
        } else {
            Err("No measurements function set".to_string())
        }
    }

    fn call_tag_keys(
        &self,
        bucket: String,
    ) -> Result<JsFuture, String> {
        if let Some(cb) = self.tag_keys.clone() {
            let promise = cb.call1(bucket.into())?;
            Ok(JsFuture::from(promise))
        } else {
            Err("No tag keys function set".to_string())
        }
    }

    fn call_tag_values(
        &self,
        bucket: String,
        field: String,
    ) -> Result<JsFuture, String> {
        if let Some(cb) = self.tag_values.clone() {
            let promise = cb.call2(bucket.into(), field.into())?;
            Ok(JsFuture::from(promise))
        } else {
            Err("No tag keys function set".to_string())
        }
    }

    pub async fn get_buckets(&self) -> Result<Vec<String>, String> {
        let mut finished = Resolvable::default();
        let mut cloned = finished.clone();
        let cln = self.clone();

        spawn_local(async move {
            let future = cln.call_buckets();
            match future {
                Ok(future) => {
                    if let Ok(returned) = future.await {
                        if let Ok(v) = returned.into_serde() {
                            cloned.resolve(v);
                        } else {
                            cloned.resolve(vec![]);
                        }
                    } else {
                        cloned.resolve(vec![]);
                    }
                }
                Err(_) => cloned.resolve(vec![]),
            }
        });

        finished.clone().await;
        Ok(finished.result())
    }

    pub async fn get_measurements(
        &self,
        bucket: String,
    ) -> Result<Vec<String>, String> {
        let mut finished = Resolvable::default();
        let mut cloned = finished.clone();
        let cln = self.clone();

        spawn_local(async move {
            let future = cln.call_measurements(bucket);
            match future {
                Ok(future) => {
                    if let Ok(returned) = future.await {
                        if let Ok(v) = returned.into_serde() {
                            cloned.resolve(v);
                        } else {
                            cloned.resolve(vec![]);
                        }
                    } else {
                        cloned.resolve(vec![]);
                    }
                }
                Err(_) => cloned.resolve(vec![]),
            }
        });

        finished.clone().await;
        Ok(finished.result())
    }

    pub async fn get_tag_keys(
        &self,
        bucket: String,
    ) -> Result<Vec<String>, String> {
        let mut finished = Resolvable::default();
        let mut cloned = finished.clone();
        let cln = self.clone();

        spawn_local(async move {
            let future = cln.call_tag_keys(bucket);
            match future {
                Ok(future) => {
                    if let Ok(returned) = future.await {
                        if let Ok(v) = returned.into_serde() {
                            cloned.resolve(v);
                        } else {
                            cloned.resolve(vec![]);
                        }
                    } else {
                        cloned.resolve(vec![]);
                    }
                }
                Err(_) => cloned.resolve(vec![]),
            }
        });

        finished.clone().await;
        Ok(finished.result())
    }

    pub async fn get_tag_values(
        &self,
        bucket: String,
        field: String,
    ) -> Result<Vec<String>, String> {
        let mut finished = Resolvable::default();
        let mut cloned = finished.clone();
        let cln = self.clone();

        spawn_local(async move {
            let future = cln.call_tag_values(bucket, field);
            match future {
                Ok(future) => {
                    if let Ok(returned) = future.await {
                        if let Ok(v) = returned.into_serde() {
                            cloned.resolve(v);
                        } else {
                            cloned.resolve(vec![]);
                        }
                    } else {
                        cloned.resolve(vec![]);
                    }
                }
                Err(_) => cloned.resolve(vec![]),
            }
        });

        finished.clone().await;
        Ok(finished.result())
    }
}
