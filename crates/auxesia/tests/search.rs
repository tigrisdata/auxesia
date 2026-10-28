use auxesia::server::App;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{fs, sync::Arc};
use tempfile::TempDir;
use tower::ServiceExt;

fn put(root: &TempDir, path: &str, text: &str) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

async fn get(app: &Arc<App>, path: &str) -> (StatusCode, String) {
    let res = app
        .router()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn search(app: &Arc<App>, query: &str) -> Value {
    let (status, body) = get(app, &format!("/_auxesia/search.json?q={query}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn search_ranks_titles_headings_and_body_and_requires_every_term() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "title.md",
        "---\ntitle: Quasar\n---\nReference manual.",
    );
    put(
        &root,
        "heading.md",
        "---\ntitle: Reference\n---\n# Quasar\nReference manual.",
    );
    put(
        &root,
        "body.md",
        "---\ntitle: Reference\n---\nQuasar reference manual.",
    );
    let app = App::open(root.path(), false).unwrap();
    let result = search(&app, "QUASAR+manual").await;
    let urls: Vec<_> = result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["url"].as_str().unwrap())
        .collect();
    assert_eq!(urls, ["/title.md", "/heading.md", "/body.md"]);
    assert_eq!(search(&app, "quasar+missing").await["total"], 0);
}

#[tokio::test]
async fn search_includes_hidden_pages_but_not_assets_and_deduplicates_aliases() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        ".auxesia.yaml",
        "ignore: ['draft.md']\nfooter: 'footersecret'\n",
    );
    put(
        &root,
        "guide/index.md",
        "---\ntitle: Guide\nindex: false\n---\nUniquequasar",
    );
    put(&root, "draft.md", "---\ntitle: Draft\n---\nExcludedquasar");
    put(&root, "static/file.md", "Excludedquasar");
    put(
        &root,
        "skills/deploy/SKILL.md",
        "---\nname: Deploy\n---\nSkillquasar",
    );
    put(&root, "skills/deploy/ref.md", "Excludedquasar");
    let app = App::open(root.path(), false).unwrap();
    let result = search(&app, "uniquequasar").await;
    assert_eq!(result["total"], 1);
    assert_eq!(result["results"][0]["url"], "/guide/");
    assert_eq!(search(&app, "excludedquasar").await["total"], 0);
    assert_eq!(search(&app, "footersecret").await["total"], 0);
    assert_eq!(
        search(&app, "skillquasar").await["results"][0]["url"],
        "/skills/deploy"
    );
}

#[tokio::test]
async fn search_matches_phrases_unicode_and_code_without_indexing_scripts() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "page.md",
        "---\ntitle: Example\n---\nCafé naïve. red blue green.\n\n```rust\nfn fetch_object() {}\n```\n\n<script>scriptsecret</script><style>stylesecret</style>\n\n<div>Visiblehtml &amp; text</div>",
    );
    let app = App::open(root.path(), false).unwrap();
    for q in ["%22red+blue%22", "CAF%C3%89", "fetch_object", "visiblehtml"] {
        assert_eq!(search(&app, q).await["total"], 1, "{q}");
    }
    for q in ["%22red+green%22", "scriptsecret", "stylesecret"] {
        assert_eq!(search(&app, q).await["total"], 0, "{q}");
    }
    let result = search(&app, "visiblehtml").await;
    assert!(
        !result["results"][0]["excerpt"]
            .as_str()
            .unwrap()
            .contains("<div>")
    );
}

#[tokio::test]
async fn phrases_respect_heading_boundaries_and_inline_text_adjacency() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "page.md",
        "---\ntitle: Sample\n---\n# Scarlet\n\nIntervening unrelated content.\n\n# Ibis\n\nfoo**bar**\n\n# Northern **pintail**\n",
    );
    let app = App::open(root.path(), false).unwrap();
    assert_eq!(search(&app, "%22scarlet+ibis%22").await["total"], 0);
    assert_eq!(search(&app, "foobar").await["total"], 1);
    assert_eq!(search(&app, "%22foo+bar%22").await["total"], 0);
    assert_eq!(search(&app, "%22northern+pintail%22").await["total"], 1);
}

#[test]
fn unchanged_search_content_reuses_index_but_reads_current_markdown() {
    let root = TempDir::new().unwrap();
    put(&root, "page.md", "---\ntitle: Example\n---\nQuasar");
    let before = auxesia::snapshot::load(root.path(), None).unwrap();
    put(&root, "static/file.txt", "unrelated");
    let after = auxesia::snapshot::load(root.path(), Some(&before)).unwrap();
    assert!(Arc::ptr_eq(&before.snapshot.search, &after.snapshot.search));
    put(&root, "page.md", "---\ntitle: Example\n---\n**Quasar**");
    let formatted = auxesia::snapshot::load(root.path(), Some(&after)).unwrap();
    assert!(Arc::ptr_eq(
        &after.snapshot.search,
        &formatted.snapshot.search
    ));
    let auxesia::render::Route::Page { markdown, .. } = &formatted.snapshot.routes["/page.md"]
    else {
        panic!("page")
    };
    assert!(String::from_utf8_lossy(markdown).contains("**Quasar**"));
}

#[tokio::test]
async fn search_head_and_method_rules_match_get() {
    let root = TempDir::new().unwrap();
    put(&root, "page.md", "---\ntitle: Example\n---\nQuasar");
    let app = App::open(root.path(), false).unwrap();
    for path in [
        "/_auxesia/search?q=quasar",
        "/_auxesia/search.json?q=quasar",
    ] {
        let get = app
            .router()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let headers = get.headers().clone();
        let head = app
            .router()
            .oneshot(Request::head(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers(), &headers);
        assert_eq!(head.headers()["cache-control"], "no-store");
        assert!(
            head.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );
        let post = app
            .router()
            .oneshot(Request::post(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[tokio::test]
async fn search_updates_and_retains_previous_snapshot_after_invalid_edit() {
    let root = TempDir::new().unwrap();
    put(&root, "page.md", "---\ntitle: Example\n---\nOldquasar");
    let app = App::open(root.path(), false).unwrap();
    assert_eq!(search(&app, "oldquasar").await["total"], 1);
    put(&root, "page.md", "---\ntitle: Example\n---\nNewquasar");
    assert_eq!(search(&app, "newquasar").await["total"], 1);
    assert_eq!(search(&app, "oldquasar").await["total"], 0);
    put(&root, "page.md", "---\ntitle: [invalid]\n---\nBrokenquasar");
    assert_eq!(search(&app, "newquasar").await["total"], 1);
    assert_eq!(search(&app, "brokenquasar").await["total"], 0);
    fs::remove_file(root.path().join("page.md")).unwrap();
    assert_eq!(search(&app, "newquasar").await["total"], 0);
    put(&root, "added.md", "---\ntitle: Added\n---\nAddedquasar");
    assert_eq!(search(&app, "addedquasar").await["total"], 1);
}

#[tokio::test]
async fn search_validates_input_and_serves_escaped_html_with_pagination() {
    let root = TempDir::new().unwrap();
    for name in ["a", "b", "c"] {
        put(
            &root,
            &format!("{name}.md"),
            "---\ntitle: '<img src=x onerror=alert(1)>'\n---\nMatchquasar",
        );
    }
    let app = App::open(root.path(), false).unwrap();
    let (status, body) = get(&app, "/_auxesia/search?q=matchquasar&limit=1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(body.contains("offset=1"));
    assert!(!body.contains("<img src=x"));
    let result = search(&app, "matchquasar&limit=1&offset=1").await;
    assert_eq!(result["total"], 3);
    assert_eq!(result["results"][0]["url"], "/b.md");
    for suffix in [
        "q=%22unfinished",
        "q=x&limit=0",
        "q=x&limit=51",
        "q=x&offset=-1",
        "q=x&offset=99999999999999999999999999",
    ] {
        assert_eq!(
            get(&app, &format!("/_auxesia/search.json?{suffix}"))
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{suffix}"
        );
    }
    assert_eq!(
        get(
            &app,
            &format!("/_auxesia/search.json?q={}", "x".repeat(513))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(search(&app, "").await["total"], 0);
}

#[tokio::test]
#[ignore = "manual release-mode benchmark; no timing assertions"]
async fn benchmark_1000_pages() {
    use std::time::Instant;
    let root = TempDir::new().unwrap();
    let paragraph = "Store objects in buckets. Configure lifecycle rules, versioning, access policies, and regional placement. Retrieve an object by its key and verify its checksum. ";
    let mut source_bytes = 0;
    for i in 0..1000 {
        let source = format!(
            "---\ntitle: Object storage guide {i}\ndescription: Reference for storage operation {i}\n---\n# Object operations\n\n{}\n\n## Example\n\n```sh\naws s3 cp file.txt s3://bucket/key{i}\n```\n",
            paragraph.repeat(20)
        );
        source_bytes += source.len();
        put(&root, &format!("guide{i:04}.md"), &source);
    }
    let start = Instant::now();
    let app = App::open(root.path(), false).unwrap();
    let startup = start.elapsed();
    let mut timings = Vec::new();
    for i in 0..100 {
        let query = if i % 2 == 0 {
            "object+storage"
        } else {
            "%22access+policies%22"
        };
        let start = Instant::now();
        // The generated root listing also contains the page titles.
        let expected = if i % 2 == 0 { 1001 } else { 1000 };
        assert_eq!(search(&app, query).await["total"], expected);
        timings.push(start.elapsed());
    }
    timings.sort();
    put(
        &root,
        "guide0500.md",
        "---\ntitle: Updated\ndescription: Updated guide\n---\nUniqueupdatequasar",
    );
    let start = Instant::now();
    assert_eq!(search(&app, "uniqueupdatequasar").await["total"], 1);
    let rebuild = start.elapsed();
    println!(
        "1000 pages, {source_bytes} source bytes; startup={startup:?}; HTTP search (including refresh) p50={:?}, p95={:?}; edit-to-result={rebuild:?}",
        timings[50], timings[95]
    );
}
