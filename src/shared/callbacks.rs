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

    pub fn call1(&self, ss: String) -> Result<Promise, String> {
        match self.f.call1(&JsValue::NULL, &JsValue::from_str(&ss)) {
            Ok(result) => Ok(Promise::from(result)),
            Err(_e) => Err("Callback failed".to_string()),
        }
    }
}

unsafe impl<'a> Send for Callback {}
unsafe impl<'a> Sync for Callback {}

enum CallbackType {
    Buckets,
    Measurements { bucket: String },
}

#[derive(Default, Clone)]
pub struct Callbacks {
    pub buckets: Option<Callback>,
    pub measurements: Option<Callback>,
}

impl Callbacks {
    pub fn register_buckets_callback(&mut self, f: Function) {
        self.buckets = Some(Callback::new(f));
    }

    pub fn register_measurements_callback(&mut self, f: Function) {
        self.measurements = Some(Callback::new(f));
    }

    pub async fn get_bucket(&self) -> Result<Vec<String>, String> {
        self.get(CallbackType::Buckets).await
    }

    pub async fn get_measurement(
        &self,
        bucket: String,
    ) -> Result<Vec<String>, String> {
        self.get(CallbackType::Measurements { bucket: bucket })
            .await
    }

    fn call_js(&self, typ: CallbackType) -> Result<JsFuture, String> {
        let promise: Promise;
        match typ {
            CallbackType::Buckets => {
                if let Some(cb) = &self.buckets {
                    promise = cb.call0()?
                } else {
                    return Err("No buckets function set".to_string());
                }
            }
            CallbackType::Measurements { bucket: b } => {
                if let Some(cb) = &self.measurements {
                    promise = cb.call1(b)?
                } else {
                    return Err(
                        "No measurements function set".to_string()
                    );
                }
            }
        }
        Ok(JsFuture::from(promise))
    }

    async fn get(
        &self,
        typ: CallbackType,
    ) -> Result<Vec<String>, String> {
        let mut finished = Resolvable::default();

        let mut cloned = finished.clone();

        let cln = self.clone();

        spawn_local(async move {
            let future = cln.call_js(typ).unwrap();
            if let Ok(returned) = future.await {
                if let Ok(v) = returned.into_serde() {
                    cloned.resolve(v);
                } else {
                    cloned.resolve(vec![]);
                }
            } else {
                cloned.resolve(vec![]);
            }
        });

        finished.clone().await;

        Ok(finished.result())
    }
}
