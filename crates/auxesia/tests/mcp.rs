use auxesia::server::App;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

fn put(root: &TempDir, path: &str, content: impl AsRef<[u8]>) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const DEPLOY: &str = "---\nname: Deploy\ndescription: Deploy a service\n---\n# Deploy\nSee [checks](references/checks.md).";

fn site() -> TempDir {
    let root = TempDir::new().unwrap();
    put(&root, "index.md", "---\ntitle: Home\n---\nHome");
    put(&root, "skills/deploy/SKILL.md", DEPLOY);
    put(&root, "skills/deploy/references/checks.md", "# Checks\n");
    put(
        &root,
        "skills/deploy/logo.png",
        [0x89, b'P', b'N', b'G', 0xff, 0xfe],
    );
    put(
        &root,
        "skills/inspect.md",
        "---\nname: Inspect\n---\n# Inspect",
    );
    put(
        &root,
        "skills/secret.md",
        "---\nname: Secret\nindex: false\n---\n# Secret",
    );
    root
}

struct Client {
    router: Router,
    session: Option<String>,
    next: u64,
}

impl Client {
    async fn connect(router: Router) -> Self {
        let mut client = Self {
            router,
            session: None,
            next: 0,
        };
        let init = client
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": { "name": "test", "version": "0" },
                }),
            )
            .await;
        assert_eq!(init["result"]["serverInfo"]["name"], "auxesia");
        let capabilities = &init["result"]["capabilities"];
        assert!(capabilities["tools"].is_object());
        assert!(capabilities["prompts"].is_null());
        assert!(capabilities["resources"].is_null());
        let (status, _) = client
            .post(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        client
    }

    async fn post(&mut self, message: Value) -> (StatusCode, Vec<u8>) {
        let mut req = Request::post("/mcp")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25");
        if let Some(session) = &self.session {
            req = req.header("mcp-session-id", session);
        }
        let res = self
            .router
            .clone()
            .oneshot(req.body(Body::from(message.to_string())).unwrap())
            .await
            .unwrap();
        if let Some(session) = res.headers().get("mcp-session-id") {
            self.session = Some(session.to_str().unwrap().to_owned());
        }
        let status = res.status();
        (
            status,
            res.into_body().collect().await.unwrap().to_bytes().to_vec(),
        )
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let message =
            json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params });
        let (status, body) = self.post(message).await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        serde_json::from_slice(&body).unwrap()
    }

    /// Returns the text of each content item and whether the result is an error.
    async fn call(&mut self, tool: &str, arguments: Value) -> (Vec<String>, bool) {
        let res = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        let result = &res["result"];
        let texts = result["content"]
            .as_array()
            .unwrap_or_else(|| panic!("no content: {res}"))
            .iter()
            .map(|c| c["text"].as_str().unwrap().to_owned())
            .collect();
        (texts, result["isError"].as_bool().unwrap_or(false))
    }
}

#[tokio::test]
async fn lists_read_only_tools() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let res = client.request("tools/list", json!({})).await;
    let tools = res["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "list_skills",
            "get_skill",
            "read_skill_file",
            "search_docs",
            "read_doc"
        ]
    );
    for tool in tools {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
    }
}

#[tokio::test]
async fn documentation_search_matches_http_and_reads_original_markdown() {
    let root = site();
    let source = "---\ntitle: Hidden quasar\nindex: false\n---\n# Quasar\nAn exact phrase.";
    put(&root, "guide/index.md", source);
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let (texts, error) = client
        .call("search_docs", json!({"query": "quasar", "limit": 1}))
        .await;
    assert!(!error, "{texts:?}");
    let results: Value = serde_json::from_str(&texts[0]).unwrap();
    let res = app
        .router()
        .oneshot(
            Request::get("/_auxesia/search.json?q=quasar&limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let http: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(results, http);
    assert_eq!(results["results"][0]["url"], "/guide/");
    for path in ["/guide/", "/guide/index.md", "/guide"] {
        let (texts, error) = client.call("read_doc", json!({"path":path})).await;
        assert!(!error, "{texts:?}");
        let doc: Value = serde_json::from_str(&texts[0]).unwrap();
        assert_eq!(doc["markdown"], source);
        assert_eq!(doc["url"], "/guide/");
        assert_eq!(doc["title"], "Hidden quasar");
    }
    let (texts, error) = client
        .call("read_doc", json!({"path":"/skills/deploy/SKILL.md"}))
        .await;
    assert!(!error, "{texts:?}");
    assert_eq!(
        serde_json::from_str::<Value>(&texts[0]).unwrap()["markdown"],
        DEPLOY
    );
}

#[tokio::test]
async fn documentation_tools_validate_queries_and_page_paths() {
    let root = site();
    put(
        &root,
        "caf%C3%A9.md",
        "---\ntitle: Percent\n---\nLiteral percent",
    );
    put(
        &root,
        "café.md",
        "---\ntitle: Unicode\n---\nUnicode content",
    );
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    for args in [
        json!({}),
        json!({"query": 1}),
        json!({"query":"\"open"}),
        json!({"query":"x", "limit": 0}),
        json!({"query":"x", "limit": 51}),
        json!({"query":"x", "offset": -1}),
        json!({"query":"x".repeat(513)}),
    ] {
        assert!(client.call("search_docs", args.clone()).await.1, "{args}");
    }
    for path in [
        "https://example.com/",
        "//example.com/",
        "guide/index.md",
        "/../index.md",
        "/%2e%2e/index.md",
        "/%2Findex.md",
        "/%FF",
        "/%xy",
        "/missing.md",
        "/skills/deploy/references/checks.md",
        "/_auxesia/style.css",
        "/index.md?x=1",
    ] {
        assert!(
            client.call("read_doc", json!({"path": path})).await.1,
            "{path}"
        );
    }
    for (path, title) in [
        ("/caf%C3%A9.md", "Unicode"),
        ("/caf%25C3%25A9.md", "Percent"),
    ] {
        let (texts, error) = client.call("read_doc", json!({"path":path})).await;
        assert!(!error, "{texts:?}");
        assert_eq!(
            serde_json::from_str::<Value>(&texts[0]).unwrap()["title"],
            title
        );
    }
}

#[tokio::test]
async fn list_skills_omits_hidden_skills() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let (texts, error) = client.call("list_skills", json!({})).await;
    assert!(!error);
    let list: Value = serde_json::from_str(&texts[0]).unwrap();
    assert_eq!(
        list,
        json!([
            {
                "slug": "deploy",
                "name": "Deploy",
                "description": "Deploy a service",
                "page": "/skills/deploy",
                "archive": "/skills/deploy.skill",
            },
            {
                "slug": "inspect",
                "name": "Inspect",
                "page": "/skills/inspect",
                "archive": "/skills/inspect.skill",
            },
        ])
    );
}

#[tokio::test]
async fn get_skill_returns_original_source_and_files() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let (texts, error) = client.call("get_skill", json!({ "name": "deploy" })).await;
    assert!(!error);
    assert_eq!(texts[0], DEPLOY);
    let about: Value = serde_json::from_str(&texts[1]).unwrap();
    assert_eq!(about["files"], json!(["logo.png", "references/checks.md"]));
    // Hidden skills stay reachable by name, as their direct URLs do.
    let (texts, error) = client.call("get_skill", json!({ "name": "secret" })).await;
    assert!(!error);
    assert!(texts[0].contains("# Secret"));
}

#[tokio::test]
async fn read_skill_file_reads_only_text_files_in_the_bundle() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let (texts, error) = client
        .call(
            "read_skill_file",
            json!({ "name": "deploy", "path": "references/checks.md" }),
        )
        .await;
    assert!(!error);
    assert_eq!(texts, ["# Checks\n"]);
    let cases = [
        (json!({ "name": "deploy", "path": "missing.md" }), "no file"),
        (
            json!({ "name": "deploy", "path": "../inspect.md" }),
            "no file",
        ),
        (
            json!({ "name": "deploy", "path": "logo.png" }),
            "/skills/deploy.skill",
        ),
        (json!({ "name": "nope", "path": "SKILL.md" }), "No skill"),
        (json!({ "name": "deploy" }), "\"path\" argument"),
        (
            json!({ "name": 1, "path": "SKILL.md" }),
            "\"name\" argument",
        ),
    ];
    for (arguments, message) in cases {
        let (texts, error) = client.call("read_skill_file", arguments.clone()).await;
        assert!(error, "{arguments}");
        assert!(texts[0].contains(message), "{arguments}: {texts:?}");
    }
}

#[tokio::test]
async fn unknown_tool_is_a_protocol_error() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let res = client
        .request(
            "tools/call",
            json!({ "name": "run_skill", "arguments": {} }),
        )
        .await;
    assert_eq!(res["error"]["code"], -32602, "{res}");
}

#[tokio::test]
async fn tools_see_edits_without_a_restart() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let changed = DEPLOY.replace("# Deploy", "# Deploy v2");
    put(&root, "skills/deploy/SKILL.md", &changed);
    let (texts, _) = client.call("get_skill", json!({ "name": "deploy" })).await;
    assert_eq!(texts[0], changed);
}

#[tokio::test]
async fn sessions_survive_across_routers() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    client.router = app.router();
    let (_, error) = client.call("list_skills", json!({})).await;
    assert!(!error);
}

#[tokio::test]
async fn rejects_foreign_browser_origins() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let res = app
        .router()
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header("origin", "https://attacker.example")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[test]
fn content_cannot_use_mcp_routes() {
    let root = TempDir::new().unwrap();
    put(&root, "index.md", "---\ntitle: Home\n---\nHome");
    put(&root, "mcp", "shadowed");
    let failure = App::open(root.path(), false).err().unwrap();
    assert!(
        failure.errors.iter().any(|e| e.contains("/mcp: reserved")),
        "{:?}",
        failure.errors
    );
}
