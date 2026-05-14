use js_sys::{Function, Object, Promise, Reflect};
use rq_lib::error::RqError;
use rq_lib::http::{HttpClient, HttpResponse};
use rq_lib::syntax::Request;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

struct SendFuture<F: Future>(F);

unsafe impl<F: Future> Send for SendFuture<F> {}

impl<F: Future> Future for SendFuture<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: wasm32 is single-threaded; no cross-thread access can occur
        unsafe { self.map_unchecked_mut(|s| &mut s.0) }.poll(cx)
    }
}

pub struct WasmHttpClient;

impl HttpClient for WasmHttpClient {
    fn execute<'a>(
        &'a self,
        request: &'a Request,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RqError>> + Send + 'a>> {
        let url = request.url.clone();
        let method = request.method.as_str().to_string();
        let headers = request.headers.clone();
        let body = request.body.clone();
        let timeout = request.timeout.clone();
        Box::pin(SendFuture(async move {
            fetch(&url, &method, &headers, body.as_deref(), timeout.as_deref()).await
        }))
    }
}

async fn fetch(
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&str>,
    timeout: Option<&str>,
) -> Result<HttpResponse, RqError> {
    let global = js_sys::global();
    let fetch_val = Reflect::get(&global, &JsValue::from_str("fetch"))
        .map_err(|_| RqError::Generic("Failed to access global fetch".to_string()))?;

    if fetch_val.is_undefined() || fetch_val.is_null() {
        return Err(RqError::Generic(
            "fetch is not available. Node.js 18+ or a browser environment is required.".to_string(),
        ));
    }

    let abort_controller = timeout
        .and_then(|t| t.parse::<f64>().ok())
        .filter(|secs| secs.is_finite() && *secs >= 0.0)
        .map(|secs| {
            let controller = js_sys::eval("new AbortController()")
                .ok()
                .filter(|v| !v.is_undefined() && !v.is_null());
            (controller, (secs * 1000.0) as i32)
        });

    let opts = Object::new();
    Reflect::set(
        &opts,
        &JsValue::from_str("method"),
        &JsValue::from_str(method),
    )
    .map_err(|_| RqError::Generic("Failed to set request method".to_string()))?;

    let headers_obj = Object::new();
    for (k, v) in headers {
        Reflect::set(&headers_obj, &JsValue::from_str(k), &JsValue::from_str(v))
            .map_err(|_| RqError::Generic(format!("Failed to set header '{k}'")))?;
    }
    Reflect::set(&opts, &JsValue::from_str("headers"), &headers_obj)
        .map_err(|_| RqError::Generic("Failed to set request headers".to_string()))?;

    if let Some(b) = body {
        Reflect::set(&opts, &JsValue::from_str("body"), &JsValue::from_str(b))
            .map_err(|_| RqError::Generic("Failed to set request body".to_string()))?;
    }

    if let Some((Some(ref controller), ms)) = abort_controller {
        let signal = Reflect::get(controller, &JsValue::from_str("signal"))
            .map_err(|_| RqError::Generic("Failed to get AbortController signal".to_string()))?;
        Reflect::set(&opts, &JsValue::from_str("signal"), &signal)
            .map_err(|_| RqError::Generic("Failed to set fetch signal".to_string()))?;

        let abort_fn = Reflect::get(controller, &JsValue::from_str("abort"))
            .ok()
            .and_then(|v| v.dyn_into::<Function>().ok());
        if let Some(abort) = abort_fn {
            let controller_clone = controller.clone();
            let cb = wasm_bindgen::closure::Closure::once(move || {
                let _ = abort.call0(&controller_clone);
            });
            let set_timeout = Reflect::get(&global, &JsValue::from_str("setTimeout"))
                .ok()
                .and_then(|v| v.dyn_into::<Function>().ok());
            if let Some(set_timeout_fn) = set_timeout {
                let _ = set_timeout_fn.call2(
                    &JsValue::UNDEFINED,
                    cb.as_ref(),
                    &JsValue::from_f64(ms as f64),
                );
                cb.forget();
            }
        }
    }

    let fetch_fn = Function::from(fetch_val);
    let promise_val = fetch_fn
        .call2(&JsValue::UNDEFINED, &JsValue::from_str(url), &opts)
        .map_err(|e| RqError::Generic(format!("fetch() call failed: {}", js_val_str(&e))))?;

    let response = JsFuture::from(Promise::from(promise_val))
        .await
        .map_err(|e| RqError::Generic(format!("HTTP request failed: {}", js_val_str(&e))))?;

    let status = Reflect::get(&response, &JsValue::from_str("status"))
        .map_err(|_| RqError::Generic("Failed to read response status".to_string()))?
        .as_f64()
        .ok_or_else(|| RqError::Generic("Response status is not a number".to_string()))?
        as u16;

    let response_headers = read_response_headers(&response);

    let text_fn = Reflect::get(&response, &JsValue::from_str("text"))
        .map_err(|_| RqError::Generic("Failed to get response.text method".to_string()))?;
    let text_fn = Function::from(text_fn);
    let text_promise = text_fn
        .call0(&response)
        .map_err(|_| RqError::Generic("Call to response.text() failed".to_string()))?;
    let text_val = JsFuture::from(Promise::from(text_promise))
        .await
        .map_err(|e| {
            RqError::Generic(format!("Reading response body failed: {}", js_val_str(&e)))
        })?;

    Ok(HttpResponse {
        status,
        headers: response_headers,
        body: text_val.as_string().unwrap_or_default(),
    })
}

fn read_response_headers(response: &JsValue) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let headers_obj = match Reflect::get(response, &JsValue::from_str("headers")) {
        Ok(h) => h,
        Err(_) => return map,
    };
    let entries_fn = match Reflect::get(&headers_obj, &JsValue::from_str("entries"))
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok())
    {
        Some(f) => f,
        None => return map,
    };
    let iterator = match entries_fn.call0(&headers_obj) {
        Ok(it) => it,
        Err(_) => return map,
    };
    loop {
        let next_fn = match Reflect::get(&iterator, &JsValue::from_str("next"))
            .ok()
            .and_then(|v| v.dyn_into::<Function>().ok())
        {
            Some(f) => f,
            None => break,
        };
        let result = match next_fn.call0(&iterator) {
            Ok(r) => r,
            Err(_) => break,
        };
        let done = Reflect::get(&result, &JsValue::from_str("done"))
            .map(|v| v.is_truthy())
            .unwrap_or(true);
        if done {
            break;
        }
        let value =
            Reflect::get(&result, &JsValue::from_str("value")).unwrap_or(JsValue::UNDEFINED);
        let arr = js_sys::Array::from(&value);
        let key = arr.get(0).as_string().unwrap_or_default();
        let val = arr.get(1).as_string().unwrap_or_default();
        if !key.is_empty() {
            map.insert(key, val);
        }
    }
    map
}

fn js_val_str(val: &JsValue) -> String {
    val.as_string()
        .or_else(|| {
            js_sys::JSON::stringify(val)
                .ok()
                .and_then(|s| s.as_string())
        })
        .unwrap_or_else(|| format!("{val:?}"))
}
