use crate::field::Field;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait Reader {
    async fn read_fields(&mut self) -> Option<HashMap<String, Field>>;
}
