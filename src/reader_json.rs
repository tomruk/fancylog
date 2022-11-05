use crate::{
    field::Field,
    reader::{ReadError, Reader},
    source::Source,
};
use anyhow::anyhow;
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
    async fn read_fields(&mut self) -> Result<HashMap<String, Field>, ReadError> {
        let line = self.source.read_line().await;
        if let Some(line) = line {
            let line = line.trim();
            match line.chars().nth(0) {
                Some(c) => {
                    if c != '{' {
                        println!("line: {}", line);
                        return Err(ReadError::ParseFail(anyhow!(
                            "first character was not '{{'"
                        )));
                    }
                }
                None => {
                    return Err(ReadError::ParseFail(anyhow!(
                        "couldn't access the first character. input is probably empty"
                    )))
                }
            }

            let json: Value = serde_json::from_str(line)
                .map_err(|e| ReadError::ParseFail(anyhow::Error::new(e)))?;
            let json_map = json
                .as_object()
                .ok_or(ReadError::Internal(anyhow!("json.as_object failed")))?;

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
            return Ok(map);
        }
        Err(ReadError::Eof)
    }
}
