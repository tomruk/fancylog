use crate::{field::Field, reader::Reader, source::Source};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

pub struct JsonReader {
    source: Source,
}

impl JsonReader {
    pub fn new(source: Source) -> Self {
        Self { source: source }
    }
}

#[async_trait]
impl Reader for JsonReader {
    async fn read_fields(&mut self) -> Option<HashMap<String, Field>> {
        let line = self.source.read_line().await;
        if let Some(line) = line {
            let line = line.trim();
            if line.len() == 0 || line.chars().nth(0).unwrap() != '{' {
                return None;
            }
            let json: Value = serde_json::from_str(line).unwrap();
            let json_map = json.as_object().or(None);
            if json_map.is_none() {
                return None;
            }
            let json_map = json_map.unwrap();
            let mut map = HashMap::new();
            map.reserve(json_map.len());
            for (k, v) in json_map {
                map.insert(
                    k.clone(),
                    Field {
                        name: k.clone(),
                        value: v.to_string(),
                    },
                );
            }
            return Some(map);
        }
        None
    }
}
