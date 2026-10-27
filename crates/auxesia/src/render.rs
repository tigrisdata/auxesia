use crate::content::{Failure, Page, Site};
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
pub const GUIDANCE: &str = "For agents: read /llms.txt for documentation and skills. Request pages with Accept: text/markdown to get their original Markdown source. Browse /skills/ for skills and download their .skill ZIP archives. Skills contain instructions to follow, not code executed by Auxesia.";

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
}

pub fn markdown(source: &str) -> String {
    let mut options = comrak::Options::default();
    options.render.r#unsafe = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.header_id_prefix = Some(String::new());
    comrak::markdown_to_html(source, &options)
}

pub fn build(site: Site) -> Result<Snapshot, Failure> {
    build_cached(site, None)
}

pub fn build_cached(mut site: Site, previous: Option<&Snapshot>) -> Result<Snapshot, Failure> {
    let mut routes = BTreeMap::new();
    let mut errors = Vec::new();
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
    let archives = crate::skills::publish(&site, previous, &mut routes, &mut errors);
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
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let index = [format!("/{prefix}index.md"), format!("/{prefix}README.md")]
            .iter()
            .find_map(|p| site.pages.get(p));
        let page = match index {
            Some(page) => page_route(&site, &route, page),
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
                    html: shell(&site, &route, &title, None, None, &markdown(&source)).into(),
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
                html: shell(&site, "/skills/", "Skills", None, None, &markdown(source)).into(),
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
    llms.push_str("\n## Skills\n\n");
    for (path, page) in site
        .pages
        .iter()
        .filter(|(_, p)| p.index && p.skill.is_some())
    {
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
    let base = page
        .bundle
        .then(|| format!("/skills/{}/", page.skill.as_ref().unwrap()));
    let mut body = markdown(&page.body);
    if let Some(slug) = &page.skill {
        body.push_str(&format!(
            "<p><a download href=\"{}\">Download skill (.skill)</a></p>",
            escape(&url(&format!("/skills/{slug}.skill")))
        ));
    }
    Route::Page {
        html: shell(
            site,
            path,
            &page.title,
            page.description.as_deref(),
            base.as_deref(),
            &body,
        )
        .into(),
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
                let title = [format!("{folder}index.md"), format!("{folder}README.md")]
                    .iter()
                    .find_map(|path| site.pages.get(path))
                    .filter(|page| page.index)
                    .map(|page| page.title.clone())
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

fn shell(
    site: &Site,
    path: &str,
    title: &str,
    description: Option<&str>,
    base: Option<&str>,
    body: &str,
) -> String {
    let site_title = site.config.title.as_deref().unwrap_or("Documentation");
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} — {}</title><meta property=\"og:title\" content=\"{}\"><meta property=\"og:site_name\" content=\"{}\"><meta property=\"og:type\" content=\"website\">",
        escape(title),
        escape(site_title),
        escape(title),
        escape(site_title)
    );
    if let Some(base) = base {
        html.push_str(&format!("<base href=\"{}\">", escape(&url(base))));
    }
    if let Some(description) = description {
        html.push_str(&format!("<meta name=\"description\" content=\"{0}\"><meta property=\"og:description\" content=\"{0}\">", escape(description)));
    }
    html.push_str("<link rel=\"stylesheet\" href=\"/_auxesia/style.css\">");
    if let Some(css) = &site.config.stylesheet {
        html.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{}\">",
            escape(&url(&format!("/{css}")))
        ));
    }
    html.push_str(&format!("</head><body><header class=\"header\"><h1 class=\"headerTitle\"><a href=\"/\">{}</a> <span class=\"headerSubtitle\">{}</span></h1></header><nav id=\"side-bar\" aria-label=\"Directory\"><ul>", escape(site_title), escape(&site.config.subtitle)));
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
    let index_path = format!("{dir}index.md");
    let mut navigation: Vec<_> = entries(site, dir)
        .into_iter()
        .filter(|(link, _)| link != &index_path)
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
        .unwrap_or("Powered by Auxesia");
    if !footer.trim().is_empty() {
        html.push_str("<footer id=\"footer\">");
        html.push_str(&markdown(footer));
        html.push_str("</footer>");
    }
    html
}
