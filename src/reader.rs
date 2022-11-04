use crate::field::Field;
use async_trait::async_trait;
use std::{collections::HashMap, error::Error, fmt::Display};

#[async_trait]
pub trait Reader {
    async fn read_fields(&mut self) -> Result<HashMap<String, Field>, ReadError>;
}

#[derive(Debug)]
pub enum ReadError {
    ParseFail,
    InternalError,
}

impl Error for ReadError {}

impl Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ParseFail => "parse fail",
            Self::InternalError => "internal error",
        };
        write!(f, "read error: {}", s)
    }
}
