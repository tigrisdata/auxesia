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
async fn lists_three_read_only_tools() {
    let root = site();
    let app = App::open(root.path(), false).unwrap();
    let mut client = Client::connect(app.router()).await;
    let res = client.request("tools/list", json!({})).await;
    let tools = res["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["list_skills", "get_skill", "read_skill_file"]);
    for tool in tools {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
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
