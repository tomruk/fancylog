use crate::{field::Field, source::SourceType};
use async_trait::async_trait;
use std::{collections::HashMap, error::Error, fmt::Display};

#[async_trait]
pub trait Reader {
    fn source_type(&self) -> SourceType;
    async fn read_fields(&mut self) -> Result<HashMap<String, Field>, ReadError>;
}

#[derive(Debug)]
pub enum ReadError {
    ParseFail(anyhow::Error),
    Internal(anyhow::Error),
    Eof,
}

impl PartialEq for ReadError {
    fn eq(&self, other: &Self) -> bool {
        use ReadError::*;
        match (self, other) {
            (&ParseFail(ref e1), &ParseFail(ref e2)) => true,
            (&Internal(ref e1), &Internal(ref e2)) => true,
            (&Eof, &Eof) => true,
            _ => false,
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl Error for ReadError {}

impl Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ParseFail(ref e) => format!("parse fail: {}", e.to_string()),
            Self::Internal(ref e) => format!("internal error: {}", e.to_string()),
            Self::Eof => "no input received (EOF)".to_string(),
        };
        write!(f, "read error: {}", s)
    }
}
