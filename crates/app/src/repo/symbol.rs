use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::symbol::{Region, Symbol, SymbolIdentifier};

#[async_trait]
pub trait SymbolRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Symbol>>;
    async fn find_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Symbol>>;
    async fn find_by_code(&self, region: &Region, code: &str) -> Result<Option<Symbol>>;
    async fn find_active(&self) -> Result<Vec<Symbol>>;
    async fn find_identifiers(&self, symbol_id: Uuid) -> Result<Vec<SymbolIdentifier>>;
}
