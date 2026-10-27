use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(deserialize_with = "optional_string")]
    pub title: Option<String>,
    #[serde(deserialize_with = "string")]
    pub subtitle: String,
    #[serde(deserialize_with = "optional_string")]
    pub footer: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub stylesheet: Option<String>,
    #[serde(deserialize_with = "strings")]
    pub ignore: Vec<String>,
    pub llms: Llms,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Llms {
    #[serde(deserialize_with = "string")]
    pub header: String,
}

pub(crate) fn optional_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    match serde_yaml_ng::Value::deserialize(deserializer)? {
        serde_yaml_ng::Value::String(value) => Ok(Some(value)),
        serde_yaml_ng::Value::Null => Ok(None),
        _ => Err(serde::de::Error::custom("expected a string")),
    }
}

fn string<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    optional_string(deserializer)?
        .ok_or_else(|| serde::de::Error::custom("expected a string, not null"))
}

fn strings<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Vec::<serde_yaml_ng::Value>::deserialize(deserializer)?
        .into_iter()
        .map(|value| match value {
            serde_yaml_ng::Value::String(value) => Ok(value),
            _ => Err(serde::de::Error::custom("expected a list of strings")),
        })
        .collect()
}

impl Config {
    pub fn ignores(&self) -> Result<GlobSet, String> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &self.ignore {
            builder.add(
                GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .build()
                    .map_err(|e| {
                        format!(".auxesia.yaml: invalid ignore pattern {pattern:?}: {e}")
                    })?,
            );
        }
        builder.build().map_err(|e| e.to_string())
    }
}

pub fn ignored(set: &GlobSet, path: &str) -> bool {
    set.is_match(path)
        || path
            .match_indices('/')
            .any(|(i, _)| set.is_match(&path[..i]))
}
