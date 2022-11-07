use std::collections::HashMap;

use crate::{
    field::Field,
    reader::{ReadError, Reader},
    source::Source,
};
use anyhow::anyhow;
use async_trait::async_trait;
use regex::Regex;

pub struct RegexReader {
    re: Regex,
    capture_names: Vec<String>,
    source: Source,
}

impl RegexReader {
    pub fn new(re: Regex, source: Source) -> Self {
        let capture_names = re
            .capture_names()
            .filter_map(|v| v.map(|x| x.to_string()))
            .collect();

        Self {
            re: re,
            capture_names: capture_names,
            source: source,
        }
    }
}

#[async_trait]
impl Reader for RegexReader {
    async fn read_fields(&mut self) -> Result<HashMap<String, Field>, ReadError> {
        let line = self.source.read_line().await;
        if let Some(line) = line {
            let line = line.trim();
            let mut map = HashMap::with_capacity(self.capture_names.len());

            #[cfg(test)]
            println!("regex: line: `{line}`");

            // TODO: Improve error?
            let caps = self
                .re
                .captures(&line)
                .ok_or(ReadError::ParseFail(anyhow!("regex doesn't match")))?;

            for name in &self.capture_names {
                match caps.name(name) {
                    Some(cap) => {
                        let cap = cap.as_str();
                        map.insert(
                            name.clone(),
                            Field {
                                name: name.clone(),
                                value: cap.to_string(),
                            },
                        );
                    }
                    None => continue,
                }
            }

            return Ok(map);
        }
        Err(ReadError::Eof)
    }
}

#[cfg(test)]
mod tests {
    use regex::Regex;
    use std::io::Cursor;
    use tokio::io::{AsyncRead, AsyncSeek, BufReader};

    use crate::{reader::Reader, source::Source};

    use super::RegexReader;

    #[tokio::test]
    async fn regex_reader() {
        let c = Cursor::new("Ela Snow\nAlice");

        let source = BufReader::new(c);
        let source = Source::new(crate::source::SourceType::File("test".to_string()), source);

        // first_name + optional space + optional last_name
        let re = Regex::new(r#"^(?P<first_name>[a-zA-Z]+)[ ]?(?P<last_name>[a-zA-Z]+)?"#).unwrap();
        let mut reader = RegexReader::new(re, source);

        let fields = reader.read_fields().await.unwrap();
        let first_name = fields.get("first_name").unwrap();
        let last_name = fields.get("last_name").unwrap();
        assert_eq!("first_name", first_name.name);
        assert_eq!("Ela", first_name.value);
        assert_eq!("last_name", last_name.name);
        assert_eq!("Snow", last_name.value);

        let fields = reader.read_fields().await.unwrap();
        let first_name = fields.get("first_name").unwrap();
        assert_eq!("first_name", first_name.name);
        assert_eq!("Alice", first_name.value);
        assert_eq!(true, fields.get("last_name").is_none());
    }
}
