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
    JsonFormat {
        exclude: Option<Exclude>,
        include: Option<Include>,
    },
    #[serde(rename = "regex")]
    RegexFormat { format: String },
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
