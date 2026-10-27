use crate::{
    content::Failure,
    render::{self, Route},
    snapshot::{self, ServingState},
};
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::Response,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};
use tokio::sync::watch;

pub struct App {
    root: PathBuf,
    dev: bool,
    state: RwLock<Arc<ServingState>>,
    flight: Mutex<Option<watch::Receiver<bool>>>,
}

impl App {
    pub fn open(root: &Path, dev: bool) -> Result<Arc<Self>, Failure> {
        let root = root.canonicalize().map_err(|e| Failure {
            errors: vec![format!("{}: {e}", root.display())],
            warnings: vec![],
        })?;
        let state = snapshot::load(&root, None)?;
        for warning in &state.warnings {
            eprintln!("warning: {warning}");
        }
        Ok(Arc::new(Self {
            root,
            dev,
            state: RwLock::new(Arc::new(state)),
            flight: Mutex::new(None),
        }))
    }

    pub fn router(self: &Arc<Self>) -> Router {
        Router::new().fallback(handle).with_state(self.clone())
    }

    async fn refresh(self: &Arc<Self>) -> Arc<ServingState> {
        let mut completion = {
            let mut flight = self.flight.lock().unwrap();
            if let Some(receiver) = flight.as_ref().filter(|receiver| !*receiver.borrow()) {
                receiver.clone()
            } else {
                let (sender, receiver) = watch::channel(false);
                *flight = Some(receiver.clone());
                let app = self.clone();
                // The task owns completion even if the initiating HTTP request is cancelled.
                tokio::spawn(async move {
                    let previous = app.state.read().unwrap().clone();
                    let root = app.root.clone();
                    let prior = previous.clone();
                    let result =
                        tokio::task::spawn_blocking(move || snapshot::load(&root, Some(&prior)))
                            .await;
                    let next = match result {
                        Ok(Ok(next)) => next,
                        other => {
                            let mut next = (*previous).clone();
                            next.errors = vec![format!("Cannot refresh documentation: {other:?}")];
                            next
                        }
                    };
                    for warning in &next.warnings {
                        if !previous.warnings.contains(warning) {
                            eprintln!("warning: {warning}");
                        }
                    }
                    for error in &next.errors {
                        if !previous.errors.contains(error) {
                            eprintln!("error: {error}");
                        }
                    }
                    *app.state.write().unwrap() = Arc::new(next);
                    let _ = sender.send(true);
                });
                receiver
            }
        };
        let _ = completion.wait_for(|done| *done).await;
        self.state.read().unwrap().clone()
    }
}

async fn handle(
    State(app): State<Arc<App>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        let mut res = response(
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain; charset=utf-8",
            b"Method not allowed\n".to_vec(),
            false,
        );
        res.headers_mut()
            .insert(header::ALLOW, "GET, HEAD".parse().unwrap());
        return res;
    }
    let head = method == Method::HEAD;
    let state = app.refresh().await;
    let Some(path) = decode_path(uri.path()) else {
        return response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            b"Invalid path\n".to_vec(),
            head,
        );
    };
    let Some(route) = state.snapshot.routes.get(&path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"Not found\n".to_vec(),
            head,
        );
    };
    match route {
        Route::Redirect(target) => {
            let location = format!(
                "{}{}",
                render::url(target),
                uri.query().map(|q| format!("?{q}")).unwrap_or_default()
            );
            let mut res = response(StatusCode::TEMPORARY_REDIRECT, "text/plain", vec![], head);
            res.headers_mut()
                .insert(header::LOCATION, location.parse().unwrap());
            res
        }
        Route::Blob {
            bytes,
            content_type,
            download,
        } => {
            let mut res = response(StatusCode::OK, content_type, bytes.to_vec(), head);
            if let Some(name) = download {
                res.headers_mut()
                    .insert(header::CONTENT_DISPOSITION, attachment(name));
            }
            res
        }
        Route::Page { html, markdown } => {
            let accept = headers
                .get_all(header::ACCEPT)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .collect::<Vec<_>>()
                .join(",");
            let mut res = match negotiate(&accept) {
                Some(true) => response(
                    StatusCode::OK,
                    "text/markdown; charset=utf-8",
                    markdown.to_vec(),
                    head,
                ),
                Some(false) => {
                    let mut body = html.to_string();
                    if app.dev && !state.errors.is_empty() {
                        body.push_str("<aside class=\"auxesia-errors\" role=\"alert\"><strong>Update failed. Serving the last valid site.</strong><ul>");
                        for error in &state.errors {
                            body.push_str(&format!("<li>{}</li>", render::escape(error)));
                        }
                        body.push_str("</ul><p>Fix these errors and reload.</p></aside>");
                    }
                    body.push_str("</body></html>");
                    response(
                        StatusCode::OK,
                        "text/html; charset=utf-8",
                        body.into_bytes(),
                        head,
                    )
                }
                None => response(
                    StatusCode::NOT_ACCEPTABLE,
                    "text/plain",
                    b"Available representations: text/html, text/markdown\n".to_vec(),
                    head,
                ),
            };
            res.headers_mut()
                .insert(header::VARY, "Accept".parse().unwrap());
            res
        }
    }
}

fn response(status: StatusCode, content_type: &str, bytes: Vec<u8>, head: bool) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(if head {
            Body::empty()
        } else {
            Body::from(bytes)
        })
        .unwrap()
}

// RFC 6266 `attr-char`: everything else is percent-encoded in `filename*`.
const ATTR_CHAR: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'#')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'^')
    .remove(b'_')
    .remove(b'`')
    .remove(b'|')
    .remove(b'~');

// Browsers do not decode `filename`, so it gets a plain ASCII fallback and the
// exact UTF-8 name goes in `filename*` (RFC 5987).
fn attachment(name: &str) -> HeaderValue {
    let fallback: String = name
        .chars()
        .map(|c| match c {
            '"' | '\\' | '%' => '_',
            ' ' => c,
            c if c.is_ascii_graphic() => c,
            _ => '_',
        })
        .collect();
    format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{}",
        utf8_percent_encode(name, ATTR_CHAR)
    )
    .parse()
    .unwrap()
}

fn decode_path(path: &str) -> Option<String> {
    let mut decoded = Vec::new();
    for part in path.split('/') {
        // Invalid percent escapes must not silently become literal filenames.
        let bytes = part.as_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            if *byte == b'%'
                && (i + 2 >= bytes.len()
                    || !bytes[i + 1].is_ascii_hexdigit()
                    || !bytes[i + 2].is_ascii_hexdigit())
            {
                return None;
            }
        }
        let part = percent_decode_str(part).decode_utf8().ok()?;
        if part == "." || part == ".." || part.contains(['/', '\\', '\0']) {
            return None;
        }
        decoded.push(part.into_owned());
    }
    Some(decoded.join("/"))
}

// A representation's most specific matching range determines its quality.
fn negotiate(accept: &str) -> Option<bool> {
    if accept.trim().is_empty() {
        return Some(false);
    }
    let quality = |kind: &str| {
        let mut best = (-1_i32, 0_u16);
        for range in accept.split(',') {
            let mut fields = range.trim().split(';');
            let media = fields.next().unwrap_or("").trim().to_ascii_lowercase();
            let specificity = if media == kind {
                2
            } else if media == "text/*" {
                1
            } else if media == "*/*" {
                0
            } else {
                continue;
            };
            let mut q = 1000;
            let mut matches = true;
            for parameter in fields {
                let Some((key, value)) = parameter.trim().split_once('=') else {
                    matches = false;
                    break;
                };
                if key.trim().eq_ignore_ascii_case("q") {
                    q = value
                        .trim()
                        .parse::<f32>()
                        .ok()
                        .filter(|q| q.is_finite() && (0.0..=1.0).contains(q))
                        .map(|q| (q * 1000.0).round() as u16)
                        .unwrap_or(0);
                } else if !key.trim().eq_ignore_ascii_case("charset")
                    || !value.trim().trim_matches('"').eq_ignore_ascii_case("utf-8")
                {
                    matches = false;
                }
            }
            if matches && (specificity > best.0 || specificity == best.0 && q > best.1) {
                best = (specificity, q);
            }
        }
        best.1
    };
    let html = quality("text/html");
    let md = quality("text/markdown");
    if html == 0 && md == 0 {
        None
    } else {
        Some(md > html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    #[tokio::test]
    async fn concurrent_requests_share_refresh_even_if_initiator_is_cancelled() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.md");
        std::fs::write(&path, "---\ntitle: Original\n---\nOriginal").unwrap();
        let app = App::open(root.path(), true).unwrap();
        std::fs::write(&path, "---\ntitle: Changed\n---\nChanged").unwrap();
        let mut first = Box::pin(app.refresh());
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(first.as_mut().poll(&mut context), Poll::Pending));
        let mut followers: Vec<_> = (0..8).map(|_| Box::pin(app.refresh())).collect();
        for follower in &mut followers {
            assert!(matches!(
                follower.as_mut().poll(&mut context),
                Poll::Pending
            ));
        }
        // The worker must outlive the request that started it.
        drop(first);
        let mut states = Vec::new();
        for follower in followers {
            states.push(follower.await);
        }
        assert!(
            states
                .windows(2)
                .all(|pair| Arc::ptr_eq(&pair[0], &pair[1]))
        );
        let Route::Page { html, .. } = &states[0].snapshot.routes["/"] else {
            panic!("page");
        };
        assert!(html.contains("Changed"));
    }
}
