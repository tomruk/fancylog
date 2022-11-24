use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub formats: HashMap<String, Format>,
    pub default_format: Option<String>,
    #[serde(default)]
    pub path_matches: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Format {
    #[serde(rename = "json")]
    JsonFormat { fields: Fields },
    #[serde(rename = "regex")]
    RegexFormat { format: String, fields: Fields },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Exclude {
    ExcludeOne(String),
    ExcludeMany(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Include {
    IncludeOne(String),
    IncludeMany(Vec<String>),
}

#[derive(Debug, Deserialize)]
pub struct Fields {
    pub message: Option<String>,
    pub timestamp: Option<TimestampField>,
    pub stacktrace: Option<String>,
    pub exclude: Option<Exclude>,
    pub include: Option<Include>,
}

#[derive(Debug, Deserialize)]
pub struct TimestampField {
    pub name: String,
    pub format: String,
}
