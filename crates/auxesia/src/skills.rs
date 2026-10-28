use crate::{
    content::{File, Page, Site},
    render::{Route, Snapshot, insert},
};
use comrak::{Arena, nodes::NodeValue, parse_document};
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    sync::Arc,
};
use zip::{ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Debug)]
pub struct Archive {
    fingerprint: [u8; 32],
    pub bytes: Arc<[u8]>,
}

/// A published skill as MCP clients see it. File bytes share the captured content.
#[derive(Clone, Debug)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub index: bool,
    pub page: String,
    pub files: BTreeMap<String, File>,
}

fn files<'a>(site: &'a Site, page: &'a Page) -> BTreeMap<String, &'a File> {
    if page.bundle {
        let prefix = format!("skills/{}/", page.skill.as_ref().unwrap());
        site.files
            .iter()
            .filter_map(|(path, file)| path.strip_prefix(&prefix).map(|p| (p.to_owned(), file)))
            .collect()
    } else {
        BTreeMap::from([("SKILL.md".into(), &site.files[&page.source_path])])
    }
}

pub fn publish(
    site: &Site,
    previous: Option<&Snapshot>,
    routes: &mut BTreeMap<String, Route>,
    errors: &mut Vec<String>,
) -> (BTreeMap<String, Archive>, BTreeMap<String, Skill>) {
    let mut archives = BTreeMap::new();
    let mut skills = BTreeMap::new();
    for (path, page) in &site.pages {
        let Some(slug) = &page.skill else {
            continue;
        };
        let mut aliases = BTreeSet::from([format!("/skills/{slug}.md")]);
        if page.bundle {
            aliases.extend([
                format!("/skills/{slug}/SKILL.md"),
                format!("/skills/{slug}/"),
            ]);
        } else {
            aliases.insert(format!("/{}", page.source_path));
        }
        for alias in aliases {
            insert(routes, alias, Route::Redirect(path.clone()), errors);
        }
        let entries = files(site, page);
        let mut hash = Sha256::new();
        hash.update(slug.as_bytes());
        hash.update([0]);
        for (path, file) in &entries {
            hash.update(path.as_bytes());
            hash.update([0, file.executable as u8]);
            hash.update((file.bytes.len() as u64).to_le_bytes());
            hash.update(&file.bytes);
        }
        let fingerprint = hash.finalize().into();
        let archive = match previous
            .and_then(|s| s.archives.get(slug))
            .filter(|a| a.fingerprint == fingerprint)
        {
            Some(archive) => archive.clone(),
            None => match pack(slug, &entries) {
                Ok(bytes) => Archive {
                    fingerprint,
                    bytes: bytes.into(),
                },
                Err(e) => {
                    errors.push(format!(
                        "{}: cannot build skill archive: {e}",
                        page.source_path
                    ));
                    continue;
                }
            },
        };
        insert(
            routes,
            format!("/skills/{slug}.skill"),
            Route::Blob {
                bytes: archive.bytes.clone(),
                content_type: "application/zip".into(),
                download: Some(format!("{slug}.skill")),
            },
            errors,
        );
        archives.insert(slug.clone(), archive);
        skills.insert(
            slug.clone(),
            Skill {
                name: page.title.clone(),
                description: page.description.clone(),
                index: page.index,
                page: path.clone(),
                files: entries.into_iter().map(|(p, f)| (p, f.clone())).collect(),
            },
        );
    }
    (archives, skills)
}

fn pack(slug: &str, entries: &BTreeMap<String, &File>) -> Result<Vec<u8>, zip::result::ZipError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, file) in entries {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(if file.executable { 0o755 } else { 0o644 });
        writer.start_file(format!("{slug}/{path}"), options)?;
        writer.write_all(&file.bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}

pub fn warnings(site: &Site) -> Vec<String> {
    let mut warnings = Vec::new();
    for page in site.pages.values().filter(|p| p.skill.is_some()) {
        let entries = files(site, page);
        for (path, file) in &entries {
            if !path.to_ascii_lowercase().ends_with(".md") {
                continue;
            }
            let Ok(source) = std::str::from_utf8(&file.bytes) else {
                continue;
            };
            let arena = Arena::new();
            let mut options = comrak::Options::default();
            options.extension.front_matter_delimiter = Some("---".into());
            let document = parse_document(&arena, source, &options);
            for node in document.descendants() {
                let data = node.data.borrow();
                let (NodeValue::Link(link) | NodeValue::Image(link)) = &data.value else {
                    continue;
                };
                let target = link.url.split(['#', '?']).next().unwrap_or("");
                if target.is_empty() || target.starts_with("//") || target.contains(':') {
                    continue;
                }
                let target = percent_decode_str(target).decode_utf8_lossy();
                let mut parts: Vec<&str> = path.split('/').collect();
                parts.pop();
                let mut outside = target.starts_with('/');
                for part in target.split('/') {
                    match part {
                        "" | "." => {}
                        ".." => {
                            if parts.pop().is_none() {
                                outside = true;
                            }
                        }
                        value => parts.push(value),
                    }
                }
                if outside || !entries.contains_key(&parts.join("/")) {
                    warnings.push(format!("{} ({path}): local link {:?} is not included in the .skill archive; use a self-contained skill bundle", page.source_path, link.url));
                }
            }
        }
    }
    warnings.sort();
    warnings.dedup();
    warnings
}
