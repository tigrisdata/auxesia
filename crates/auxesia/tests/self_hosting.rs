use auxesia::{content, render, server::App};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::path::Path;
use tower::ServiceExt;

/// The shipped documentation is a real site fixture, including a downloadable skill.
#[tokio::test]
async fn serves_its_own_documentation_without_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let site = content::build(&root).unwrap();
    let snapshot = render::build(content::build(&root).unwrap()).unwrap();
    assert!(snapshot.warnings.is_empty(), "{:?}", snapshot.warnings);
    assert!(site.pages.contains_key("/getting-started.md"));
    assert!(site.pages.contains_key("/skills/serve-auxesia.md"));
    assert!(!snapshot.routes.contains_key("/plans/auxesia-v1.md"));
    assert!(!snapshot.routes.contains_key("/plans/"));
    for page in site.pages.values() {
        let arena = comrak::Arena::new();
        let document = comrak::parse_document(&arena, &page.body, &comrak::Options::default());
        for node in document.descendants() {
            let data = node.data.borrow();
            let (comrak::nodes::NodeValue::Link(link) | comrak::nodes::NodeValue::Image(link)) =
                &data.value
            else {
                continue;
            };
            let target = link.url.split(['#', '?']).next().unwrap();
            if target.is_empty() || target.contains(':') || target.starts_with("//") {
                continue;
            }
            let target = percent_encoding::percent_decode_str(target)
                .decode_utf8()
                .unwrap();
            let mut parts: Vec<&str> = if target.starts_with('/') {
                vec![]
            } else {
                let mut parts: Vec<_> = page.source_path.split('/').collect();
                parts.pop();
                parts
            };
            for part in target.split('/') {
                match part {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    value => parts.push(value),
                }
            }
            let route = format!(
                "/{}{}",
                parts.join("/"),
                if target.ends_with('/') && !parts.is_empty() {
                    "/"
                } else {
                    ""
                }
            );
            assert!(
                snapshot.routes.contains_key(&route),
                "{} links to missing route {route}",
                page.source_path
            );
        }
    }
    let app = App::open(&root, true).unwrap();
    for (path, page) in &site.pages {
        let response = app
            .router()
            .oneshot(
                Request::builder()
                    .uri(render::url(path))
                    .header("accept", "text/markdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.as_ref(), page.source.as_ref(), "{path}");
    }
    for (path, expected_type) in [
        ("/", "text/html; charset=utf-8"),
        ("/reference/", "text/html; charset=utf-8"),
        ("/llms.txt", "text/plain; charset=utf-8"),
        ("/skills/serve-auxesia.skill", "application/zip"),
    ] {
        let response = app
            .router()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.headers()["content-type"], expected_type);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        if path == "/" {
            let html = std::str::from_utf8(&body).unwrap();
            assert!(html.contains("display: none"));
            assert!(!html.contains("auxesia-errors"));
        } else if path.ends_with(".skill") {
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(body)).unwrap();
            assert!(zip.by_name("serve-auxesia/SKILL.md").is_ok());
            assert!(zip.len() >= 2);
        }
    }
}
