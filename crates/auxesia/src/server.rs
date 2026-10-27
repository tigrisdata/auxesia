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
use bytes::Bytes;
use http_body::{Frame, SizeHint};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::{
    collections::VecDeque,
    convert::Infallible,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll},
};
use tokio::sync::watch;

pub struct App {
    root: PathBuf,
    dev: bool,
    state: RwLock<Arc<ServingState>>,
    flight: Mutex<Option<watch::Receiver<Phase>>>,
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
        // An abandoned refresh published nothing, so the request starts another one.
        for _ in 0..3 {
            let mut completion = self.join_or_start();
            if let Ok(phase) = completion
                .wait_for(|phase| matches!(phase, Phase::Published | Phase::Abandoned))
                .await
                && *phase == Phase::Published
            {
                break;
            }
        }
        self.state.read().unwrap().clone()
    }

    fn join_or_start(self: &Arc<Self>) -> watch::Receiver<Phase> {
        let mut flight = self.flight.lock().unwrap();
        // A queued refresh has not scanned yet, so it sees every edit made before this request.
        // A scanning refresh may have read older files, so a new refresh runs after it.
        let predecessor = match flight.as_ref() {
            Some(receiver) if receiver.has_changed().is_err() => None,
            Some(receiver) if *receiver.borrow() == Phase::Queued => return receiver.clone(),
            Some(receiver) if *receiver.borrow() == Phase::Scanning => Some(receiver.clone()),
            _ => None,
        };
        let (sender, receiver) = watch::channel(Phase::Queued);
        *flight = Some(receiver.clone());
        let signal = Signal(sender);
        let app = self.clone();
        // The task owns completion even if the initiating HTTP request is cancelled.
        tokio::spawn(async move {
            if let Some(mut predecessor) = predecessor {
                let _ = predecessor
                    .wait_for(|phase| matches!(phase, Phase::Published | Phase::Abandoned))
                    .await;
            }
            signal.0.send_replace(Phase::Scanning);
            let previous = app.state.read().unwrap().clone();
            let root = app.root.clone();
            let prior = previous.clone();
            let result =
                tokio::task::spawn_blocking(move || snapshot::load(&root, Some(&prior))).await;
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
            signal.0.send_replace(Phase::Published);
        });
        receiver
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Queued,
    Scanning,
    Published,
    Abandoned,
}

// Waiters must always learn the outcome, even if the worker panics or is dropped before it runs.
struct Signal(watch::Sender<Phase>);

impl Drop for Signal {
    fn drop(&mut self) {
        self.0.send_if_modified(|phase| {
            let abandoned = *phase != Phase::Published;
            if abandoned {
                *phase = Phase::Abandoned;
            }
            abandoned
        });
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
            vec![Bytes::from_static(b"Method not allowed\n")],
            false,
        );
        res.headers_mut()
            .insert(header::ALLOW, "GET, HEAD".parse().unwrap());
        return res;
    }
    let head = method == Method::HEAD;
    let Some(path) = decode_path(uri.path()) else {
        return response(
            StatusCode::BAD_REQUEST,
            "text/plain",
            vec![Bytes::from_static(b"Invalid path\n")],
            head,
        );
    };
    // An invalid path never names a file, so it does not need a scan.
    let state = app.refresh().await;
    let Some(route) = state.snapshot.routes.get(&path) else {
        return response(
            StatusCode::NOT_FOUND,
            "text/plain",
            vec![Bytes::from_static(b"Not found\n")],
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
            let mut res = response(StatusCode::OK, content_type, vec![shared(bytes)], head);
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
                    vec![shared(markdown)],
                    head,
                ),
                Some(false) => {
                    let mut body = vec![shared(&Arc::<[u8]>::from(html.clone()))];
                    if app.dev && !state.errors.is_empty() {
                        let mut aside = String::from(
                            "<aside class=\"auxesia-errors\" role=\"alert\"><strong>Update failed. Serving the last valid site.</strong><ul>",
                        );
                        for error in &state.errors {
                            aside.push_str(&format!("<li>{}</li>", render::escape(error)));
                        }
                        aside.push_str("</ul><p>Fix these errors and reload.</p></aside>");
                        body.push(Bytes::from(aside));
                    }
                    body.push(Bytes::from_static(b"</body></html>"));
                    response(StatusCode::OK, "text/html; charset=utf-8", body, head)
                }
                None => response(
                    StatusCode::NOT_ACCEPTABLE,
                    "text/plain",
                    vec![Bytes::from_static(
                        b"Available representations: text/html, text/markdown\n",
                    )],
                    head,
                ),
            };
            res.headers_mut()
                .insert(header::VARY, "Accept".parse().unwrap());
            res
        }
    }
}

// Bodies are lists of chunks so cached content is sent without copying it.
fn response(status: StatusCode, content_type: &str, chunks: Vec<Bytes>, head: bool) -> Response {
    let length: usize = chunks.iter().map(Bytes::len).sum();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length)
        .header(header::CACHE_CONTROL, "no-store")
        .body(if head {
            Body::empty()
        } else {
            Body::new(Chunks(chunks.into()))
        })
        .unwrap()
}

// The returned bytes keep the cached allocation alive instead of copying it.
fn shared(data: &Arc<[u8]>) -> Bytes {
    Bytes::from_owner(data.clone())
}

struct Chunks(VecDeque<Bytes>);

impl http_body::Body for Chunks {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Ready(self.0.pop_front().map(|chunk| Ok(Frame::data(chunk))))
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.0.iter().map(|chunk| chunk.len() as u64).sum())
    }
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

    #[tokio::test]
    async fn responses_share_cached_bodies_instead_of_copying() {
        use http_body_util::BodyExt;
        use tower::ServiceExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("index.md"), "---\ntitle: Home\n---\nHome").unwrap();
        std::fs::write(root.path().join("notes.txt"), "plain notes").unwrap();
        let app = App::open(root.path(), false).unwrap();
        let state = app.state.read().unwrap().clone();
        for (path, accept, cached) in [
            ("/notes.txt", "*/*", "/notes.txt"),
            ("/", "text/markdown", "/"),
            ("/", "text/html", "/"),
        ] {
            let expected = match &state.snapshot.routes[cached] {
                Route::Blob { bytes, .. } => bytes.as_ptr(),
                Route::Page { markdown, .. } if accept == "text/markdown" => markdown.as_ptr(),
                Route::Page { html, .. } => html.as_ptr(),
                Route::Redirect(_) => panic!("redirect"),
            };
            let request = axum::http::Request::builder()
                .uri(path)
                .header(header::ACCEPT, accept)
                .body(Body::empty())
                .unwrap();
            let mut body = app.router().oneshot(request).await.unwrap().into_body();
            let chunk = body.frame().await.unwrap().unwrap().into_data().unwrap();
            assert_eq!(chunk.as_ptr(), expected, "{path} {accept}");
        }
    }

    #[tokio::test]
    async fn invalid_paths_do_not_scan_the_source_tree() {
        use tower::ServiceExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("index.md"), "---\ntitle: Home\n---\nHome").unwrap();
        let app = App::open(root.path(), true).unwrap();
        let res = app
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/%zz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert!(app.flight.lock().unwrap().is_none());
    }

    fn page_html(state: &ServingState) -> &str {
        let Route::Page { html, .. } = &state.snapshot.routes["/"] else {
            panic!("page");
        };
        html
    }

    #[test]
    fn refresh_recovers_after_a_worker_dies_without_signalling() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.md");
        std::fs::write(&path, "---\ntitle: Original\n---\nOriginal").unwrap();
        let app = App::open(root.path(), true).unwrap();
        // Shutting down this runtime drops the spawned worker before it runs.
        let doomed = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        doomed.block_on(async {
            let mut refresh = Box::pin(app.refresh());
            let mut context = Context::from_waker(Waker::noop());
            assert!(matches!(refresh.as_mut().poll(&mut context), Poll::Pending));
        });
        drop(doomed);
        std::fs::write(
            &path,
            "---\ntitle: Changed\n---\nChanged after the dead worker",
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let state = runtime.block_on(app.refresh());
        assert!(page_html(&state).contains("Changed after the dead worker"));
    }

    #[tokio::test]
    async fn request_does_not_accept_a_scan_that_started_before_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.md");
        std::fs::write(&path, "---\ntitle: Original\n---\nOriginal").unwrap();
        let app = App::open(root.path(), true).unwrap();
        let mut first = Box::pin(app.refresh());
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(first.as_mut().poll(&mut context), Poll::Pending));
        // Let the worker start its scan, then give the blocking scan time to read the tree.
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(&path, "---\ntitle: Changed\n---\nSaved during the scan").unwrap();
        let state = app.refresh().await;
        assert!(page_html(&state).contains("Saved during the scan"));
        first.await;
    }
}
