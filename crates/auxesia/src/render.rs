use crate::content::{Failure, Page, Site};
use comrak::{Arena, nodes::NodeValue, parse_document};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::{collections::BTreeMap, sync::Arc};

const URL_COMPONENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'+')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');
pub const GUIDANCE: &str = "For agents: read /llms.txt for documentation and skills. Request pages with Accept: text/markdown to get their original Markdown source. Browse /skills/ for skills and download their .skill ZIP archives. Connect an MCP client to /mcp to list skills and read their files with tools. Skills contain instructions to follow, not code executed by Auxesia.";

pub fn url(path: &str) -> String {
    path.split('/')
        .map(|part| utf8_percent_encode(part, URL_COMPONENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn md_label(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(['\r', '\n'], " ")
}

#[derive(Clone, Debug)]
pub enum Route {
    Page {
        html: Arc<str>,
        markdown: Arc<[u8]>,
    },
    Blob {
        bytes: Arc<[u8]>,
        content_type: String,
        download: Option<String>,
    },
    Redirect(String),
}

#[derive(Debug)]
pub struct Snapshot {
    pub routes: BTreeMap<String, Route>,
    pub fingerprint: [u8; 32],
    pub warnings: Vec<String>,
    pub archives: BTreeMap<String, crate::skills::Archive>,
    pub skills: BTreeMap<String, crate::skills::Skill>,
}

pub fn markdown(source: &str) -> String {
    render_markdown(source, None)
}

/// Renders Markdown. With a `base`, relative link and image URLs, including
/// `href` and `src` attributes in raw HTML, are rewritten to start with it.
/// Fragment-only, query-only, absolute-path, and scheme URLs are unchanged.
fn render_markdown(source: &str, base: Option<&str>) -> String {
    let mut options = comrak::Options::default();
    options.render.r#unsafe = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.header_id_prefix = Some(String::new());
    let arena = Arena::new();
    let document = parse_document(&arena, source, &options);
    if let Some(base) = base {
        for node in document.descendants() {
            match &mut node.data.borrow_mut().value {
                NodeValue::Link(link) | NodeValue::Image(link) => {
                    if relative(&link.url) {
                        link.url.insert_str(0, base);
                    }
                }
                NodeValue::HtmlInline(html) => *html = rebase_html(html, base),
                NodeValue::HtmlBlock(block) => block.literal = rebase_html(&block.literal, base),
                _ => {}
            }
        }
    }
    let mut html = String::new();
    comrak::format_html(document, &options, &mut html).unwrap();
    html
}

fn relative(url: &str) -> bool {
    let scheme = url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    !url.is_empty() && !url.starts_with(['#', '?', '/', '\\']) && !scheme
}

/// Prefixes relative `href` and `src` attribute values in raw HTML with `base`.
/// Comments and the contents of `script` and `style` elements are copied as-is.
fn rebase_html(html: &str, base: &str) -> String {
    let space = |c: char| c.is_ascii_whitespace();
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if rest.starts_with("<!--") {
            let end = rest[4..].find("-->").map_or(rest.len(), |i| i + 7);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        if !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            out.push('<');
            rest = &rest[1..];
            continue;
        }
        let name_end = rest
            .find(|c: char| space(c) || c == '/' || c == '>')
            .unwrap_or(rest.len());
        let tag = rest[1..name_end].to_ascii_lowercase();
        out.push_str(&rest[..name_end]);
        rest = &rest[name_end..];
        loop {
            let attribute = rest.trim_start_matches(|c: char| space(c) || c == '/');
            out.push_str(&rest[..rest.len() - attribute.len()]);
            rest = attribute;
            if rest.is_empty() || rest.starts_with('>') {
                break;
            }
            // An attribute name may start with '=', so always take one character.
            let first = rest.chars().next().unwrap().len_utf8();
            let name_end = rest[first..]
                .find(|c: char| space(c) || matches!(c, '/' | '>' | '='))
                .map_or(rest.len(), |i| i + first);
            let name = &rest[..name_end];
            out.push_str(name);
            rest = &rest[name_end..];
            let Some(value) = rest.trim_start_matches(space).strip_prefix('=') else {
                continue;
            };
            let value = value.trim_start_matches(space);
            out.push_str(&rest[..rest.len() - value.len()]);
            rest = value;
            let (quote, value) = match rest.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let end = rest[1..].find(quote).map_or(rest.len(), |i| i + 1);
                    (&rest[..1], &rest[1..end])
                }
                _ => (
                    "",
                    &rest[..rest
                        .find(|c: char| space(c) || c == '>')
                        .unwrap_or(rest.len())],
                ),
            };
            out.push_str(quote);
            let url = value.trim_start_matches(space);
            if (name.eq_ignore_ascii_case("href") || name.eq_ignore_ascii_case("src"))
                && relative(url)
            {
                out.push_str(&value[..value.len() - url.len()]);
                out.push_str(base);
                out.push_str(url);
            } else {
                out.push_str(value);
            }
            rest = &rest[quote.len() + value.len()..];
            if !quote.is_empty() && rest.starts_with(quote) {
                out.push_str(quote);
                rest = &rest[1..];
            }
        }
        if tag == "script" || tag == "style" {
            let end = rest
                .to_ascii_lowercase()
                .find(&format!("</{tag}"))
                .unwrap_or(rest.len());
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

pub fn build(site: Site) -> Result<Snapshot, Failure> {
    build_cached(site, None)
}

pub fn build_cached(mut site: Site, previous: Option<&Snapshot>) -> Result<Snapshot, Failure> {
    let mut routes = BTreeMap::new();
    let mut errors = Vec::new();
    if let Err(error) = crate::favicon::generate(&site, &mut routes) {
        errors.push(error);
    }
    insert(
        &mut routes,
        "/_auxesia/style.css".into(),
        Route::Blob {
            bytes: Arc::from(include_bytes!("style.css").as_slice()),
            content_type: "text/css; charset=utf-8".into(),
            download: None,
        },
        &mut errors,
    );
    for (path, file) in &site.files {
        let route = format!("/{path}");
        if path == "llms.txt" || path.starts_with("_auxesia/") {
            errors.push(format!("{path}: reserved generated route"));
            continue;
        }
        if site.pages.values().any(|p| p.source_path == *path) {
            continue;
        }
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        insert(
            &mut routes,
            route,
            Route::Blob {
                bytes: file.bytes.clone(),
                content_type: if path.ends_with(".md") {
                    "text/markdown; charset=utf-8".into()
                } else {
                    mime
                },
                download: None,
            },
            &mut errors,
        );
    }
    for (path, page) in &site.pages {
        insert(
            &mut routes,
            path.clone(),
            page_route(&site, path, page),
            &mut errors,
        );
    }
    let (archives, skills) = crate::skills::publish(&site, previous, &mut routes, &mut errors);
    // Diagnostics are calculated independently of archive cache reuse.
    site.warnings.extend(crate::skills::warnings(&site));
    for dir in &site.dirs {
        if site
            .bundles
            .iter()
            .any(|bundle| dir == bundle || dir.starts_with(&format!("{bundle}/")))
        {
            continue;
        }
        let route = if dir.is_empty() {
            "/".into()
        } else {
            format!("/{dir}/")
        };
        let page = match directory_page(&site, &route) {
            Some((_, page)) => page_route(&site, &route, page),
            None => {
                let title = if dir.is_empty() {
                    site.config
                        .title
                        .as_deref()
                        .unwrap_or("Documentation")
                        .to_owned()
                } else {
                    dir.rsplit('/').next().unwrap().to_owned()
                };
                let mut source = format!("# {}\n\n", md_label(&title));
                for (link, label) in entries(&site, &route) {
                    source.push_str(&format!("- [{}]({})\n", md_label(&label), url(&link)));
                }
                Route::Page {
                    html: shell(&site, &route, &title, None, &markdown(&source)).into(),
                    markdown: source.into_bytes().into(),
                }
            }
        };
        insert(&mut routes, route.clone(), page, &mut errors);
        if route != "/" {
            insert(
                &mut routes,
                route.trim_end_matches('/').to_owned(),
                Route::Redirect(route),
                &mut errors,
            );
        }
    }
    if !routes.contains_key("/skills/") {
        let source = "# Skills\n\nNo skills are published.\n";
        insert(
            &mut routes,
            "/skills/".into(),
            Route::Page {
                html: shell(&site, "/skills/", "Skills", None, &markdown(source)).into(),
                markdown: Arc::from(source.as_bytes()),
            },
            &mut errors,
        );
        insert(
            &mut routes,
            "/skills".into(),
            Route::Redirect("/skills/".into()),
            &mut errors,
        );
    }
    let mut llms = format!(
        "# {}\n\n{}\n\n{}\n\n## Documentation\n\n",
        md_label(site.config.title.as_deref().unwrap_or("Documentation")),
        site.config.llms.header,
        GUIDANCE
    );
    for (path, page) in site
        .pages
        .iter()
        .filter(|(_, p)| p.index && p.skill.is_none())
    {
        llms.push_str(&discovery_entry(path, page));
    }
    let mut indexed_skills = site
        .pages
        .iter()
        .filter(|(_, p)| p.index && p.skill.is_some())
        .peekable();
    if indexed_skills.peek().is_some() {
        llms.push_str("\n## Skills\n\n");
    }
    for (path, page) in indexed_skills {
        llms.push_str(&discovery_entry(path, page));
        llms.push_str(&format!(
            "  - [Download skill]({})\n",
            url(&format!("/skills/{}.skill", page.skill.as_ref().unwrap()))
        ));
    }
    insert(
        &mut routes,
        "/llms.txt".into(),
        Route::Blob {
            bytes: llms.into_bytes().into(),
            content_type: "text/plain; charset=utf-8".into(),
            download: None,
        },
        &mut errors,
    );
    // The MCP endpoint answers these paths before the documentation routes.
    for path in crate::mcp::ROUTES {
        if routes.contains_key(path) {
            errors.push(format!("{path}: reserved for the MCP endpoint"));
        }
    }
    if !errors.is_empty() {
        return Err(Failure {
            errors,
            warnings: site.warnings,
        });
    }
    Ok(Snapshot {
        routes,
        fingerprint: site.fingerprint,
        warnings: site.warnings,
        archives,
        skills,
    })
}

fn discovery_entry(path: &str, page: &Page) -> String {
    format!(
        "- [{}]({}){}\n",
        md_label(&page.title),
        url(path),
        page.description
            .as_ref()
            .map(|d| format!(": {}", d.replace(['\r', '\n'], " ")))
            .unwrap_or_default()
    )
}

/// Returns the page that supplies the directory route `dir` (ending in `/`):
/// `index.md` takes precedence over `README.md`.
fn directory_page<'a>(site: &'a Site, dir: &str) -> Option<(&'a String, &'a Page)> {
    ["index.md", "README.md"]
        .iter()
        .find_map(|name| site.pages.get_key_value(&format!("{dir}{name}")))
}

pub(crate) fn insert(
    routes: &mut BTreeMap<String, Route>,
    path: String,
    route: Route,
    errors: &mut Vec<String>,
) {
    if routes.insert(path.clone(), route).is_some() {
        errors.push(format!("conflicting route: {path}"));
    }
}

fn page_route(site: &Site, path: &str, page: &Page) -> Route {
    // Bundle pages resolve relative URLs against the bundle folder.
    let base = page
        .bundle
        .then(|| url(&format!("/skills/{}/", page.skill.as_ref().unwrap())));
    let mut body = String::new();
    if let Some(slug) = &page.skill {
        body.push_str(&format!(
            "<p><a class=\"skill-download\" download href=\"{}\">Download skill (.skill)</a></p>",
            escape(&url(&format!("/skills/{slug}.skill")))
        ));
    }
    body.push_str(&render_markdown(&page.body, base.as_deref()));
    Route::Page {
        html: shell(site, path, &page.title, page.description.as_deref(), &body).into(),
        markdown: page.source.clone(),
    }
}

fn entries(site: &Site, dir: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    for (path, page) in &site.pages {
        if !page.index {
            continue;
        }
        if let Some(rest) = path.strip_prefix(dir) {
            if let Some((child, _)) = rest.split_once('/') {
                let folder = format!("{dir}{child}/");
                let title = directory_page(site, &folder)
                    .filter(|(_, page)| page.index)
                    .map(|(_, page)| page.title.clone())
                    .unwrap_or_else(|| {
                        let words = child
                            .split(['-', '_'])
                            .filter(|word| !word.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let mut chars = words.chars();
                        match chars.next() {
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                            None => child.to_owned(),
                        }
                    });
                entries.insert(folder, title);
            } else {
                entries.insert(path.clone(), page.title.clone());
            }
        }
    }
    entries
}

fn shell(site: &Site, path: &str, title: &str, description: Option<&str>, body: &str) -> String {
    let site_title = site.config.title.as_deref().unwrap_or("Documentation");
    let browser_title = if title == site_title {
        title.to_owned()
    } else {
        format!("{title} — {site_title}")
    };
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><meta property=\"og:title\" content=\"{}\"><meta property=\"og:site_name\" content=\"{}\"><meta property=\"og:type\" content=\"website\">",
        escape(&browser_title),
        escape(title),
        escape(site_title)
    );
    if let Some(description) = description {
        html.push_str(&format!("<meta name=\"description\" content=\"{0}\"><meta property=\"og:description\" content=\"{0}\">", escape(description)));
    }
    html.push_str("<link rel=\"stylesheet\" href=\"/_auxesia/style.css\">");
    if site.config.favicon.is_some() {
        html.push_str(crate::favicon::LINKS);
    }
    if let Some(css) = &site.config.stylesheet {
        html.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{}\">",
            escape(&url(&format!("/{css}")))
        ));
    }
    html.push_str("</head><body><header class=\"header\"><h1 class=\"headerTitle\"><a href=\"/\">");
    if site.config.favicon.is_some() {
        html.push_str(
            "<img class=\"headerIcon\" src=\"/.auxesia/gen/favicon-32x32.png\" alt=\"\"> ",
        );
    }
    html.push_str(&format!("{}</a> <span class=\"headerSubtitle\">{}</span></h1></header><nav id=\"side-bar\" aria-label=\"Directory\"><ul>", escape(site_title), escape(&site.config.subtitle)));
    let dir = if path.ends_with('/') {
        path
    } else {
        &path[..path.rfind('/').unwrap() + 1]
    };
    html.push_str(&format!("<li><a href=\"{}\">.</a></li>", escape(&url(dir))));
    if dir != "/" {
        let trimmed = dir.trim_end_matches('/');
        let parent = &trimmed[..trimmed.rfind('/').unwrap() + 1];
        html.push_str(&format!(
            "<li><a href=\"{}\">..</a></li>",
            escape(&url(parent))
        ));
    }
    let directory = directory_page(site, dir).map(|(link, _)| link);
    let mut navigation: Vec<_> = entries(site, dir)
        .into_iter()
        .filter(|(link, _)| Some(link) != directory)
        .collect();
    navigation.sort_by_cached_key(|(link, title)| {
        (
            !link.ends_with('/'),
            title.to_lowercase(),
            title.clone(),
            link.clone(),
        )
    });
    for (link, label) in navigation {
        let icon = if link.ends_with('/') {
            "<svg class=\"folder-icon\" aria-hidden=\"true\" focusable=\"false\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" stroke-linejoin=\"round\"><path d=\"M3 7V4h6l3 3h9v13H3Z\"/></svg>"
        } else {
            ""
        };
        html.push_str(&format!(
            "<li><a href=\"{}\">{icon}{}</a></li>",
            escape(&url(&link)),
            escape(&label)
        ));
    }
    html.push_str("</ul></nav><main id=\"main\">");
    html.push_str(body);
    if path == "/" {
        html.push_str(&format!(
            "<div style=\"display: none\">{}</div>",
            escape(GUIDANCE)
        ));
    }
    html.push_str("</main>");
    let footer = site
        .config
        .footer
        .as_deref()
        .unwrap_or("Powered by [Auxesia](https://github.com/tigrisdata/auxesia), with ❤️ from the nerds at [Tigris](https://www.tigrisdata.com)");
    if !footer.trim().is_empty() {
        html.push_str("<footer id=\"footer\">");
        html.push_str(&markdown(footer));
        html.push_str("</footer>");
    }
    html
}
