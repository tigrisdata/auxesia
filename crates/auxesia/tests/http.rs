use auxesia::server::App;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::{fs, sync::Arc};
use tempfile::TempDir;
use tower::ServiceExt;

fn put(root: &TempDir, path: &str, content: &str) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
async fn request(
    app: &Arc<App>,
    path: &str,
    accept: Option<&str>,
    method: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(path).method(method);
    if let Some(accept) = accept {
        req = req.header("accept", accept);
    }
    let res = app
        .router()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = res.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes().to_vec(),
    )
}

#[tokio::test]
async fn footer_supports_markdown_defaults_and_removal() {
    let root = TempDir::new().unwrap();
    let source = "---\ntitle: Home\n---\nHome body";
    put(&root, "index.md", source);
    let app = App::open(root.path(), true).unwrap();
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(html.contains("Powered by <a href=\"https://github.com/tigrisdata/auxesia\">Auxesia</a>, with ❤️ from the nerds at <a href=\"https://www.tigrisdata.com\">Tigris</a>"));
    put(
        &root,
        ".auxesia.yaml",
        "footer: '**Our docs** · [Contact](/contact.md)'\n",
    );
    for path in ["/", "/index.md", "/skills/"] {
        let html = String::from_utf8(request(&app, path, None, "GET").await.2).unwrap();
        let footer = html
            .split("<footer id=\"footer\">")
            .nth(1)
            .unwrap()
            .split("</footer>")
            .next()
            .unwrap();
        assert!(footer.contains("<strong>Our docs</strong>"));
        assert!(footer.contains("<a href=\"/contact.md\">Contact</a>"));
        assert!(!footer.contains("Powered by"));
    }
    assert_eq!(
        request(&app, "/index.md", Some("text/markdown"), "GET")
            .await
            .2,
        source.as_bytes()
    );
    put(&root, ".auxesia.yaml", "footer: 42\n");
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(html.contains("<strong>Our docs</strong>"));
    assert!(html.contains("auxesia-errors"));
    put(&root, ".auxesia.yaml", "footer: ''\n");
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(!html.contains("<footer"));
    assert!(!html.contains("auxesia-errors"));
}

#[tokio::test]
async fn header_uses_dot_for_index_and_sorts_links_by_title() {
    let root = TempDir::new().unwrap();
    for prefix in ["", "guide/"] {
        put(
            &root,
            &format!("{prefix}index.md"),
            "---\ntitle: Directory home\n---\nHome",
        );
        put(
            &root,
            &format!("{prefix}a.md"),
            "---\ntitle: Zebra\n---\nZebra",
        );
        put(
            &root,
            &format!("{prefix}z.md"),
            "---\ntitle: apple\n---\nApple",
        );
    }
    let app = App::open(root.path(), false).unwrap();
    for path in ["/", "/a.md", "/guide/", "/guide/a.md"] {
        let html = String::from_utf8(request(&app, path, None, "GET").await.2).unwrap();
        let nav = html
            .split("<nav ")
            .nth(1)
            .unwrap()
            .split("</nav>")
            .next()
            .unwrap();
        let dir = if path.starts_with("/guide/") {
            "/guide/"
        } else {
            "/"
        };
        assert!(nav.contains(&format!("href=\"{dir}\">.</a>")));
        assert!(!nav.contains("index.md"));
        assert!(nav.find(">apple</a>").unwrap() < nav.find(">Zebra</a>").unwrap());
        assert!(nav.find(">.</a>").unwrap() < nav.find(">apple</a>").unwrap());
        if dir == "/guide/" {
            assert!(nav.find(">..</a>").unwrap() < nav.find(">apple</a>").unwrap());
        }
    }
    assert_eq!(
        request(&app, "/index.md", Some("text/markdown"), "GET")
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn header_groups_folders_and_uses_directory_titles_or_readable_names() {
    let root = TempDir::new().unwrap();
    for (path, title) in [
        ("z-folder/index.md", "Alpha & reference"),
        ("z-folder/README.md", "Unused fallback"),
        ("a-folder/README.md", "Zulu guides"),
        ("getting-started/page.md", "Child"),
        ("private-notes/index.md", "Hidden title"),
        ("private-notes/child.md", "Visible child"),
        ("z.md", "apple page"),
        ("a.md", "Zebra page"),
    ] {
        let hidden = if path == "private-notes/index.md" {
            "index: false\n"
        } else {
            ""
        };
        put(
            &root,
            path,
            &format!("---\ntitle: {title}\n{hidden}---\nBody"),
        );
    }
    let app = App::open(root.path(), false).unwrap();
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    let nav = html
        .split("<nav ")
        .nth(1)
        .unwrap()
        .split("</nav>")
        .next()
        .unwrap();
    let labels = [
        "Alpha &amp; reference",
        "Getting started",
        "Private notes",
        "Zulu guides",
        "apple page",
        "Zebra page",
    ];
    let positions: Vec<_> = labels
        .iter()
        .map(|label| {
            nav.find(label)
                .unwrap_or_else(|| panic!("missing {label}: {nav}"))
        })
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!nav.contains("Hidden title"));
    assert!(!nav.contains("Unused fallback"));
    assert_eq!(nav.matches("class=\"folder-icon\"").count(), 4);
    assert_eq!(nav.matches("aria-hidden=\"true\"").count(), 4);
    assert!(nav.find(">.</a>").unwrap() < positions[0]);
}

#[tokio::test]
async fn negotiates_exact_source_and_honors_specific_rejections() {
    let root = TempDir::new().unwrap();
    let source = "---\ntitle: Example\ndescription: An example\n---\n# Hello\n<div data-trusted>embedded</div>\n";
    put(&root, "example.md", source);
    let app = App::open(root.path(), false).unwrap();
    for (accept, expected) in [
        (None, "text/html; charset=utf-8"),
        (Some("*/*"), "text/html; charset=utf-8"),
        (Some("text/markdown"), "text/markdown; charset=utf-8"),
        (
            Some("text/markdown;q=0, */*;q=1"),
            "text/html; charset=utf-8",
        ),
        (
            Some("text/html;q=0, text/*;q=1"),
            "text/markdown; charset=utf-8",
        ),
        (
            Some("text/html;q=0.5, text/markdown;q=0.9"),
            "text/markdown; charset=utf-8",
        ),
    ] {
        let (status, headers, body) = request(&app, "/example.md", accept, "GET").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], expected);
        assert_eq!(headers["vary"], "Accept");
        if expected.starts_with("text/markdown") {
            assert_eq!(body, source.as_bytes());
        } else {
            let html = String::from_utf8(body).unwrap();
            assert!(html.contains("<div data-trusted>embedded</div>"));
            assert!(html.contains("property=\"og:title\" content=\"Example\""));
            assert!(!html.contains("title: Example"));
        }
        let (_, head_headers, head_body) = request(&app, "/example.md", accept, "HEAD").await;
        assert!(head_body.is_empty());
        assert_eq!(head_headers["content-type"], expected);
    }
    assert_eq!(
        request(&app, "/example.md", Some("application/json"), "GET")
            .await
            .0,
        StatusCode::NOT_ACCEPTABLE
    );
    assert_eq!(
        request(&app, "/example.md", None, "POST").await.0,
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[tokio::test]
async fn hides_discovery_but_not_direct_access_and_generates_indices() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        ".auxesia.yaml",
        "title: 'A & B'\nignore: [plans/**]\nllms:\n  header: Custom introduction\n",
    );
    put(
        &root,
        "hidden.md",
        "---\ntitle: Secret\nindex: false\n---\nHidden body",
    );
    put(
        &root,
        "guide/start.md",
        "---\ntitle: Start\n---\nStart here",
    );
    put(&root, "plans/private.md", "no metadata");
    let app = App::open(root.path(), false).unwrap();
    let (_, _, html) = request(&app, "/", None, "GET").await;
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains("display: none"));
    assert!(html.contains("A &amp; B"));
    assert!(!html.contains("Secret"));
    assert_eq!(
        request(&app, "/hidden.md", None, "GET").await.0,
        StatusCode::OK
    );
    for path in [
        "/plans/private.md",
        "/.auxesia.yaml",
        "/%2e%2e/private",
        "/guide%2fstart.md",
    ] {
        assert_ne!(request(&app, path, None, "GET").await.0, StatusCode::OK);
    }
    let (status, headers, _) = request(&app, "/guide?hello=world", None, "GET").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(headers["location"], "/guide/?hello=world");
    let (_, _, md) = request(&app, "/guide/", Some("text/markdown"), "GET").await;
    assert!(String::from_utf8(md).unwrap().contains("/guide/start.md"));
    let (_, _, llms) = request(&app, "/llms.txt", None, "GET").await;
    let llms = String::from_utf8(llms).unwrap();
    assert!(llms.contains("Custom introduction"));
    assert!(llms.contains("Accept: text/markdown"));
    assert!(!llms.contains("hidden.md"));
}

#[tokio::test]
async fn bundles_skills_with_original_sources_and_raw_supporting_files() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "skills/deploy/SKILL.md",
        "---\nname: Deploy & run\ndescription: Deploy a service\n---\n# Deploy\n![Diagram](diagram.svg)\n<img src=\"diagram.svg\">\n[Notes](notes.md)\n[Section](#deploy)\n",
    );
    put(
        &root,
        "skills/deploy/notes.md",
        "Supporting notes without frontmatter",
    );
    put(&root, "skills/deploy/diagram.svg", "<svg/>");
    let single = "---\nname: Inspect\n---\nInspect the service\n";
    put(&root, "skills/inspect.md", single);
    let app = App::open(root.path(), false).unwrap();
    let (_, _, body) = request(&app, "/skills/deploy.md", None, "GET").await;
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("<base href=\"/skills/deploy/\">"));
    assert!(html.contains("<img src=\"diagram.svg\">"));
    assert!(html.contains("og:title\" content=\"Deploy &amp; run\""));
    assert!(!html.contains(">notes.md</a></li>"));
    let (_, _, notes) = request(&app, "/skills/deploy/notes.md", None, "GET").await;
    assert_eq!(notes, b"Supporting notes without frontmatter");
    for path in ["/skills/deploy/", "/skills/deploy/SKILL.md"] {
        let (status, headers, _) = request(&app, path, None, "GET").await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(headers["location"], "/skills/deploy.md");
    }
    for (slug, count) in [("deploy", 3), ("inspect", 1)] {
        let path = format!("/skills/{slug}.skill");
        let (status, headers, bytes) = request(&app, &path, None, "GET").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "application/zip");
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .contains(&format!("{slug}.skill"))
        );
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(archive.len(), count);
        let mut text = String::new();
        use std::io::Read;
        archive
            .by_name(&format!("{slug}/SKILL.md"))
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        if slug == "inspect" {
            assert_eq!(text, single);
        }
        let other = App::open(root.path(), false).unwrap();
        assert_eq!(request(&other, &path, None, "GET").await.2, bytes);
    }
}

#[test]
fn ignores_complete_bundle_when_entrypoint_is_ignored_and_rejects_collisions() {
    let root = TempDir::new().unwrap();
    put(&root, ".auxesia.yaml", "ignore: [skills/deploy/SKILL.md]\n");
    put(
        &root,
        "skills/deploy/SKILL.md",
        "---\nname: Deploy\n---\nGo",
    );
    put(&root, "skills/deploy/notes.md", "No metadata");
    assert!(App::open(root.path(), false).is_ok());
    put(&root, ".auxesia.yaml", "{}");
    put(&root, "skills/deploy.md", "---\nname: Duplicate\n---\nGo");
    assert!(
        App::open(root.path(), false)
            .err()
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("conflict"))
    );
}

#[tokio::test]
async fn invalid_update_retains_whole_snapshot_until_repaired() {
    let root = TempDir::new().unwrap();
    let valid = "---\ntitle: Original\n---\nOriginal body";
    put(&root, "index.md", valid);
    put(&root, "asset.txt", "original asset");
    put(
        &root,
        "skills/task.md",
        "---\nname: Task\n---\nOriginal skill",
    );
    let app = App::open(root.path(), true).unwrap();
    let archive = request(&app, "/skills/task.skill", None, "GET").await.2;
    put(&root, "index.md", "---\ntitle: ''\n---\nBroken body");
    put(&root, "other.md", "Missing title");
    put(&root, "asset.txt", "new asset");
    put(&root, "skills/task.md", "---\nname: Task\n---\nNew skill");
    put(&root, ".auxesia.yaml", "ignore: [asset.txt]\n");
    let (_, _, html) = request(&app, "/", None, "GET").await;
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains("Original body"));
    assert!(html.contains("auxesia-errors"));
    assert!(html.contains("index.md") && html.contains("other.md"));
    assert_eq!(
        request(&app, "/index.md", Some("text/markdown"), "GET")
            .await
            .2,
        valid.as_bytes()
    );
    assert_eq!(
        request(&app, "/asset.txt", None, "GET").await.2,
        b"original asset"
    );
    assert_eq!(
        request(&app, "/skills/task.skill", None, "GET").await.2,
        archive
    );
    put(
        &root,
        "index.md",
        "---\ntitle: Repaired\n---\nRepaired body",
    );
    put(&root, "other.md", "---\ntitle: Other\n---\nOther body");
    let (_, _, html) = request(&app, "/", None, "GET").await;
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains("Repaired body"));
    assert!(!html.contains("auxesia-errors"));
    assert_eq!(
        request(&app, "/asset.txt", None, "GET").await.0,
        StatusCode::NOT_FOUND
    );
    assert_ne!(
        request(&app, "/skills/task.skill", None, "GET").await.2,
        archive
    );
}

#[tokio::test]
async fn production_hides_diagnostics_and_warning_only_edits_publish() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "index.md",
        "---\ntitle: Original\ndescription: Original\n---\nOriginal body",
    );
    let app = App::open(root.path(), false).unwrap();
    put(&root, "index.md", "---\ntitle: Changed\n---\nChanged body");
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(html.contains("Changed body"));
    assert!(!html.contains("auxesia-errors"));
    put(&root, "index.md", "bad");
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(html.contains("Changed body"));
    assert!(!html.contains("auxesia-errors"));
}

#[tokio::test]
async fn directory_aliases_prefer_index_and_hidden_index_does_not_hide_children() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "guide/index.md",
        "---\ntitle: Index title\nindex: false\n---\nIndex body",
    );
    put(
        &root,
        "guide/README.md",
        "---\ntitle: Readme title\n---\nReadme body",
    );
    put(
        &root,
        "guide/child.md",
        "---\ntitle: Child\n---\nChild body",
    );
    let app = App::open(root.path(), false).unwrap();
    let html = String::from_utf8(request(&app, "/guide/", None, "GET").await.2).unwrap();
    assert!(html.contains("Index body"));
    assert!(!html.contains("Readme body"));
    let llms = String::from_utf8(request(&app, "/llms.txt", None, "GET").await.2).unwrap();
    assert!(llms.contains("/guide/child.md"));
    assert!(!llms.contains("/guide/index.md"));
    fs::remove_file(root.path().join("guide/index.md")).unwrap();
    assert!(
        String::from_utf8(request(&app, "/guide/", None, "GET").await.2)
            .unwrap()
            .contains("Readme body")
    );
}

#[tokio::test]
async fn skill_ignore_hidden_archive_and_executable_permissions() {
    let root = TempDir::new().unwrap();
    put(&root, ".auxesia.yaml", "ignore: [skills/run/omit.txt]\n");
    put(
        &root,
        "skills/run/SKILL.md",
        "---\nname: Run\nindex: false\n---\n[Script](run.sh)\n[Missing](../missing.md)",
    );
    put(&root, "skills/run/run.sh", "#!/bin/sh\necho hello\n");
    put(&root, "skills/run/omit.txt", "omitted");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.path().join("skills/run/run.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let app = App::open(root.path(), false).unwrap();
    let llms = String::from_utf8(request(&app, "/llms.txt", None, "GET").await.2).unwrap();
    assert!(!llms.contains("/skills/run"));
    let bytes = request(&app, "/skills/run.skill", None, "GET").await.2;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(zip.len(), 2);
    assert!(zip.by_name("run/omit.txt").is_err());
    #[cfg(unix)]
    assert_eq!(
        zip.by_name("run/run.sh").unwrap().unix_mode().unwrap() & 0o777,
        0o755
    );
    assert_eq!(
        request(&app, "/skills/run/omit.txt", None, "GET").await.0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn rejects_generated_route_conflicts() {
    for path in ["llms.txt", "_auxesia/file.txt", "skills/run.skill"] {
        let root = TempDir::new().unwrap();
        put(&root, "skills/run.md", "---\nname: Run\n---\nRun");
        put(&root, path, "conflict");
        assert!(App::open(root.path(), false).is_err(), "accepted {path}");
    }
}

#[tokio::test]
async fn generated_links_encode_markdown_delimiters_and_unicode() {
    let root = TempDir::new().unwrap();
    put(&root, "part).md", "---\ntitle: Part\n---\nPart");
    put(&root, "café #1.md", "---\ntitle: Café\n---\nCoffee");
    let app = App::open(root.path(), false).unwrap();
    let html = String::from_utf8(request(&app, "/", None, "GET").await.2).unwrap();
    assert!(html.contains("href=\"/part%29.md\""));
    assert!(!html.contains("href=\"/part\""));
    let llms = String::from_utf8(request(&app, "/llms.txt", None, "GET").await.2).unwrap();
    assert!(llms.contains("/part%29.md"));
    assert_eq!(
        request(&app, "/part%29.md", None, "GET").await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "/caf%C3%A9%20%231.md", None, "GET").await.0,
        StatusCode::OK
    );
    for path in ["/bad%", "/bad%GG", "/%00", "/%5c", "/%2f"] {
        assert_eq!(
            request(&app, path, None, "GET").await.0,
            StatusCode::BAD_REQUEST
        );
    }
}
