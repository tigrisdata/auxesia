use crate::{render, server::App, skills::Skill};
use mcpkit_core::{
    capability::{ServerCapabilities, ServerInfo},
    error::McpError,
    types::{
        Content, GetPromptResult, Object, Prompt, Resource, ResourceContents, Tool,
        ToolAnnotations, ToolOutput,
    },
};
use mcpkit_server::{Context, PromptHandler, ResourceHandler, ServerHandler, ToolHandler};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// Paths that the MCP router serves. Documentation content must not use them.
pub const ROUTES: [&str; 2] = ["/mcp", "/mcp/sse"];

const INSTRUCTIONS: &str = "Call list_skills to find a skill, then get_skill to read its SKILL.md instructions and follow them. Read supporting files with read_skill_file. Auxesia only publishes skills. It never runs them.";

/// Exposes the published skills as read-only MCP tools.
pub struct Skills {
    app: Arc<App>,
}

impl Skills {
    pub fn new(app: Arc<App>) -> Self {
        Self { app }
    }
}

impl ServerHandler for Skills {
    fn server_info(&self) -> ServerInfo {
        ServerInfo::new("auxesia", env!("CARGO_PKG_VERSION"))
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities::new().with_tools()
    }

    fn instructions(&self) -> Option<String> {
        Some(INSTRUCTIONS.into())
    }
}

impl ToolHandler for Skills {
    async fn list_tools(&self, _: &Context<'_>) -> Result<Vec<Tool>, McpError> {
        Ok(vec![
            Tool::new("list_skills")
                .description("List the skills that this documentation site publishes, with their names, descriptions, and URLs.")
                .input_schema(json!({ "type": "object", "properties": {} }))
                .annotations(ToolAnnotations::read_only()),
            Tool::new("get_skill")
                .description("Get the SKILL.md instructions of a skill and the paths of its supporting files.")
                .with_string_param("name", "Skill slug from list_skills, such as serve-auxesia.", true)
                .annotations(ToolAnnotations::read_only()),
            Tool::new("read_skill_file")
                .description("Read one text file from a skill bundle, such as a reference that SKILL.md links to.")
                .with_string_param("name", "Skill slug from list_skills.", true)
                .with_string_param("path", "File path relative to the skill folder, as get_skill lists it.", true)
                .annotations(ToolAnnotations::read_only()),
        ])
    }

    async fn call_tool(
        &self,
        name: &str,
        args: Object,
        _: &Context<'_>,
    ) -> Result<ToolOutput, McpError> {
        let state = self.app.current().await;
        let skills = &state.snapshot.skills;
        let output = match name {
            "list_skills" => {
                let list: Vec<Value> = skills
                    .iter()
                    .filter(|(_, skill)| skill.index)
                    .map(|(slug, skill)| summary(slug, skill))
                    .collect();
                ToolOutput::json(&list).map_err(|e| McpError::internal(e.to_string()))?
            }
            "get_skill" => match find(skills, &args) {
                Err(output) => output,
                Ok((slug, skill)) => {
                    let entry = &skill.files["SKILL.md"];
                    let supporting: Vec<&str> = skill
                        .files
                        .keys()
                        .map(String::as_str)
                        .filter(|path| *path != "SKILL.md")
                        .collect();
                    let mut about = summary(slug, skill);
                    about["files"] = json!(supporting);
                    ToolOutput::content(vec![
                        Content::text(String::from_utf8_lossy(&entry.bytes)),
                        Content::text(
                            serde_json::to_string_pretty(&about)
                                .map_err(|e| McpError::internal(e.to_string()))?,
                        ),
                    ])
                }
            },
            "read_skill_file" => match (find(skills, &args), string(&args, "path")) {
                (Err(output), _) | (_, Err(output)) => output,
                (Ok((slug, skill)), Ok(path)) => match skill.files.get(path) {
                    None => ToolOutput::error_with_suggestion(
                        format!("Skill {slug:?} has no file {path:?}."),
                        "Call get_skill to list the files of this skill.",
                    ),
                    Some(file) => match std::str::from_utf8(&file.bytes) {
                        Ok(text) => ToolOutput::text(text),
                        Err(_) => ToolOutput::error(format!(
                            "{path:?} is not a text file. Download the whole skill from {}.",
                            archive(slug)
                        )),
                    },
                },
            },
            _ => {
                return Err(McpError::invalid_params(
                    "tools/call",
                    format!("unknown tool: {name}"),
                ));
            }
        };
        Ok(output)
    }
}

fn string<'a>(args: &'a Object, key: &str) -> Result<&'a str, ToolOutput> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolOutput::error(format!("The {key:?} argument must be a string.")))
}

// Lookup is by exact slug: hidden skills stay reachable, as their direct URLs do.
fn find<'a>(
    skills: &'a std::collections::BTreeMap<String, Skill>,
    args: &Object,
) -> Result<(&'a str, &'a Skill), ToolOutput> {
    let name = string(args, "name")?;
    skills
        .get_key_value(name)
        .map(|(slug, skill)| (slug.as_str(), skill))
        .ok_or_else(|| {
            ToolOutput::error_with_suggestion(
                format!("No skill is named {name:?}."),
                "Call list_skills to see the available skill slugs.",
            )
        })
}

fn archive(slug: &str) -> String {
    render::url(&format!("/skills/{slug}.skill"))
}

fn summary(slug: &str, skill: &Skill) -> Value {
    let mut value = Map::new();
    value.insert("slug".into(), json!(slug));
    value.insert("name".into(), json!(skill.name));
    if let Some(description) = &skill.description {
        value.insert("description".into(), json!(description));
    }
    value.insert("page".into(), json!(render::url(&skill.page)));
    value.insert("archive".into(), json!(archive(slug)));
    Value::Object(value)
}

// McpRouter requires these handlers. The server does not advertise prompts or resources.
impl PromptHandler for Skills {
    async fn list_prompts(&self, _: &Context<'_>) -> Result<Vec<Prompt>, McpError> {
        Ok(vec![])
    }

    async fn get_prompt(
        &self,
        name: &str,
        _: Option<Map<String, Value>>,
        _: &Context<'_>,
    ) -> Result<GetPromptResult, McpError> {
        Err(McpError::invalid_params(
            "prompts/get",
            format!("unknown prompt: {name}"),
        ))
    }
}

impl ResourceHandler for Skills {
    async fn list_resources(&self, _: &Context<'_>) -> Result<Vec<Resource>, McpError> {
        Ok(vec![])
    }

    async fn read_resource(
        &self,
        uri: &str,
        _: &Context<'_>,
    ) -> Result<Vec<ResourceContents>, McpError> {
        Err(McpError::resource_not_found(uri))
    }
}
