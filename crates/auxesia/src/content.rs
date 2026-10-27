use crate::config::{Config, ignored};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::Arc,
};

#[derive(Clone, Debug, Default)]
pub struct Failure {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.errors.join("\n"))
    }
}
impl std::error::Error for Failure {}

#[derive(Clone, Debug)]
pub struct File {
    pub bytes: Arc<[u8]>,
    pub executable: bool,
}

#[derive(Debug)]
pub struct Capture {
    pub config: Config,
    pub files: BTreeMap<String, File>,
    pub dirs: BTreeSet<String>,
    pub bundles: BTreeSet<String>,
    pub fingerprint: [u8; 32],
    pub errors: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub description: Option<String>,
    pub index: bool,
    pub source: Arc<[u8]>,
    pub body: String,
    pub source_path: String,
    pub skill: Option<String>,
    pub bundle: bool,
}

#[derive(Debug)]
pub struct Site {
    pub config: Config,
    pub pages: BTreeMap<String, Page>,
    pub files: BTreeMap<String, File>,
    pub dirs: BTreeSet<String>,
    pub bundles: BTreeSet<String>,
    pub fingerprint: [u8; 32],
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
struct Frontmatter {
    #[serde(default, deserialize_with = "crate::config::optional_string")]
    title: Option<String>,
    #[serde(default, deserialize_with = "crate::config::optional_string")]
    name: Option<String>,
    #[serde(default, deserialize_with = "crate::config::optional_string")]
    description: Option<String>,
    #[serde(default = "yes")]
    index: bool,
}
fn yes() -> bool {
    true
}

pub fn capture(root: &Path) -> Capture {
    let mut out = Capture {
        config: Config::default(),
        files: BTreeMap::new(),
        dirs: BTreeSet::new(),
        bundles: BTreeSet::new(),
        fingerprint: [0; 32],
        errors: Vec::new(),
    };
    let mut hash = Sha256::new();
    let config_path = root.join(".auxesia.yaml");
    match fs::symlink_metadata(&config_path) {
        Ok(meta) if meta.file_type().is_symlink() => out
            .errors
            .push(".auxesia.yaml: symlinks are not supported".into()),
        Ok(_) => match fs::read(&config_path) {
            Ok(bytes) => {
                hash.update(&bytes);
                match serde_yaml_ng::from_slice(&bytes) {
                    Ok(config) => out.config = config,
                    Err(e) => out.errors.push(format!(".auxesia.yaml: {e}")),
                }
            }
            Err(e) => out.errors.push(format!(".auxesia.yaml: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => out.errors.push(format!(".auxesia.yaml: {e}")),
    }
    if out
        .config
        .title
        .as_ref()
        .is_some_and(|t| t.trim().is_empty())
    {
        out.errors
            .push(".auxesia.yaml: title must not be blank".into());
    }
    out.config.title.get_or_insert_with(|| {
        root.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let ignore = match out.config.ignores() {
        Ok(set) => set,
        Err(e) => {
            out.errors.push(e);
            Config::default().ignores().unwrap()
        }
    };
    walk(root, "", &ignore, &mut out);
    // A folder whose files are all ignored, hidden, or absent publishes no directory route.
    let occupied: BTreeSet<&str> = out
        .files
        .keys()
        .flat_map(|path| path.match_indices('/').map(|(i, _)| &path[..i]))
        .collect();
    out.dirs
        .retain(|dir| dir.is_empty() || occupied.contains(dir.as_str()));
    for dir in &out.dirs {
        hash.update(b"D");
        hash.update(dir.as_bytes());
        hash.update([0]);
    }
    for (path, file) in &out.files {
        hash.update(b"F");
        hash.update(path.as_bytes());
        hash.update([0, file.executable as u8]);
        hash.update((file.bytes.len() as u64).to_le_bytes());
        hash.update(&file.bytes);
    }
    out.errors.sort();
    for error in &out.errors {
        hash.update(error.as_bytes());
        hash.update([0]);
    }
    out.fingerprint = hash.finalize().into();
    out
}

fn walk(root: &Path, relative: &str, ignore: &globset::GlobSet, out: &mut Capture) {
    let dir = root.join(relative);
    let entries: Vec<_> = match fs::read_dir(&dir) {
        Ok(entries) => entries.collect(),
        Err(e) => {
            out.errors.push(format!("{relative}: {e}"));
            return;
        }
    };
    // An ignored bundle entrypoint excludes the whole bundle, before Markdown classification.
    // Match the listed name exactly: a stat of SKILL.md also finds skill.md on case-insensitive filesystems.
    let entrypoint = if relative.starts_with("skills/") && relative.matches('/').count() == 1 {
        entries
            .iter()
            .flatten()
            .find(|entry| entry.file_name() == "SKILL.md")
            .and_then(|entry| fs::symlink_metadata(entry.path()).ok())
    } else {
        None
    };
    if entrypoint.is_some() && ignored(ignore, &format!("{relative}/SKILL.md")) {
        return;
    }
    let is_bundle = entrypoint.as_ref().is_some_and(|m| m.is_file());
    if entrypoint.is_some() && !is_bundle {
        out.errors.push(format!(
            "{relative}/SKILL.md: a skill entrypoint must be a regular file"
        ));
    }
    if is_bundle {
        out.bundles.insert(relative.to_owned());
    }
    out.dirs.insert(relative.to_owned());
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                out.errors.push(format!("{relative}: {e}"));
                continue;
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            out.errors
                .push(format!("{relative}: filenames must be UTF-8"));
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = if relative.is_empty() {
            name.to_owned()
        } else {
            format!("{relative}/{name}")
        };
        if ignored(ignore, &path) {
            continue;
        }
        if name.contains(['\\', '\0']) {
            out.errors.push(format!("{path}: invalid filename"));
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(e) => {
                out.errors.push(format!("{path}: {e}"));
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            out.errors
                .push(format!("{path}: symlinks are not supported"));
        } else if metadata.is_dir() {
            walk(root, &path, ignore, out);
        } else if metadata.is_file() {
            match fs::read(entry.path()) {
                Ok(bytes) => {
                    #[cfg(unix)]
                    let executable = {
                        use std::os::unix::fs::PermissionsExt;
                        metadata.permissions().mode() & 0o111 != 0
                    };
                    #[cfg(not(unix))]
                    let executable = false;
                    out.files.insert(
                        path,
                        File {
                            bytes: bytes.into(),
                            executable,
                        },
                    );
                }
                Err(e) => out.errors.push(format!("{path}: {e}")),
            }
        } else {
            out.errors
                .push(format!("{path}: only regular files are supported"));
        }
    }
}

pub fn build(root: &Path) -> Result<Site, Failure> {
    from_capture(capture(root))
}

pub fn from_capture(captured: Capture) -> Result<Site, Failure> {
    let mut failure = Failure {
        errors: captured.errors,
        warnings: vec![],
    };
    let mut pages = BTreeMap::new();
    for (path, file) in &captured.files {
        let bundle = captured
            .bundles
            .iter()
            .find(|b| path.starts_with(&format!("{b}/")));
        if let Some(bundle) = bundle
            && path != &format!("{bundle}/SKILL.md")
        {
            continue;
        }
        if !path.to_ascii_lowercase().ends_with(".md") {
            continue;
        }
        let skill = if let Some(bundle) = bundle {
            Some(bundle["skills/".len()..].to_owned())
        } else if path.starts_with("skills/")
            && path.matches('/').count() == 1
            // These names supply the directory page for /skills/, as in any other folder.
            && !matches!(path.as_str(), "skills/index.md" | "skills/README.md")
        {
            Some(path["skills/".len()..path.len() - 3].to_owned())
        } else {
            None
        };
        match parse_page(path, file, skill, bundle.is_some(), &mut failure.warnings) {
            Ok(page) => {
                let route = page
                    .skill
                    .as_ref()
                    .map(|s| format!("/skills/{s}.md"))
                    .unwrap_or_else(|| format!("/{path}"));
                if pages.insert(route.clone(), page).is_some() {
                    failure
                        .errors
                        .push(format!("{path}: conflicting route {route}"));
                }
            }
            Err(e) => failure.errors.push(e),
        }
    }
    if let Some(css) = &captured.config.stylesheet
        && (!valid_relative(css) || !captured.files.contains_key(css))
    {
        failure.errors.push(format!(
            ".auxesia.yaml: stylesheet {css:?} must name an eligible file inside the docs root"
        ));
    }
    if !failure.errors.is_empty() {
        failure.errors.sort();
        return Err(failure);
    }
    Ok(Site {
        config: captured.config,
        pages,
        files: captured.files,
        dirs: captured.dirs,
        bundles: captured.bundles,
        fingerprint: captured.fingerprint,
        warnings: failure.warnings,
    })
}

pub fn valid_relative(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && !s.starts_with('.')
                && !s.contains(['\\', '\0'])
        })
}

fn parse_page(
    path: &str,
    file: &File,
    skill: Option<String>,
    bundle: bool,
    warnings: &mut Vec<String>,
) -> Result<Page, String> {
    let source =
        std::str::from_utf8(&file.bytes).map_err(|e| format!("{path}: invalid UTF-8: {e}"))?;
    let mut lines = source.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return Err(format!(
            "{path}: add YAML frontmatter with {}",
            if skill.is_some() { "name" } else { "title" }
        ));
    }
    let mut yaml = String::new();
    let mut offset = source.find('\n').map_or(source.len(), |i| i + 1);
    let mut closed = false;
    for line in lines {
        offset += line.len();
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        yaml.push_str(line);
    }
    if !closed {
        return Err(format!("{path}: frontmatter needs a closing --- line"));
    }
    let meta: Frontmatter = serde_yaml_ng::from_str(&yaml)
        .map_err(|e| format!("{path}: frontmatter (starts at line 2): {e}"))?;
    let (title, key) = if skill.is_some() {
        (meta.name, "name")
    } else {
        (meta.title, "title")
    };
    let title = title
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("{path}: {key} must be a non-empty string"))?;
    let description = meta.description.filter(|d| !d.trim().is_empty());
    if description.is_none() {
        warnings.push(format!("{path}: missing description; add `description: A short summary of this page` to the frontmatter"));
    }
    Ok(Page {
        title,
        description,
        index: meta.index,
        source: file.bytes.clone(),
        body: source[offset..].to_owned(),
        source_path: path.to_owned(),
        skill,
        bundle,
    })
}
