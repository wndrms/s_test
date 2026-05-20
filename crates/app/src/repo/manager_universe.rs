use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::manager_universe::ManagerSymbol;

#[async_trait]
pub trait ManagerUniverseRepository: Send + Sync {
    async fn list_by_manager(&self, manager_id: Uuid) -> Result<Vec<ManagerSymbol>>;
    async fn add_symbol(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<()>;
    async fn remove_symbol(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<()>;
    async fn set_symbols(&self, manager_id: Uuid, symbol_ids: Vec<Uuid>) -> Result<()>;
}
