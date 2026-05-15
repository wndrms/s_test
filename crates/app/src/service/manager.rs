use std::sync::Arc;
use uuid::Uuid;

use lumos_domain::model::manager::Manager;
use lumos_domain::model::risk::RiskPolicy;

use crate::error::{AppError, AppResult};
use crate::repo::manager::{CreateManagerInput, ManagerRepository, RiskPolicyRepository};

pub struct ManagerService {
    managers: Arc<dyn ManagerRepository>,
    policies: Arc<dyn RiskPolicyRepository>,
}

impl ManagerService {
    pub fn new(
        managers: Arc<dyn ManagerRepository>,
        policies: Arc<dyn RiskPolicyRepository>,
    ) -> Self {
        Self { managers, policies }
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> AppResult<Vec<Manager>> {
        self.managers
            .find_by_user(user_id)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn get(&self, id: Uuid) -> AppResult<Manager> {
        self.managers
            .find_by_id(id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("manager {id}")))
    }

    pub async fn create(&self, input: CreateManagerInput) -> AppResult<Manager> {
        let manager = self
            .managers
            .create(input)
            .await
            .map_err(AppError::Internal)?;

        let default_policy = RiskPolicy::default_for(manager.id);
        self.policies
            .upsert(default_policy)
            .await
            .map_err(AppError::Internal)?;

        Ok(manager)
    }

    pub async fn get_risk_policy(&self, manager_id: Uuid) -> AppResult<RiskPolicy> {
        self.policies
            .find_by_manager(manager_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("risk policy for manager {manager_id}")))
    }

    pub async fn set_auto_trade(&self, manager_id: Uuid, enabled: bool) -> AppResult<Manager> {
        let manager = self.get(manager_id).await?;
        if !manager.is_active() {
            return Err(AppError::Forbidden("manager is not active".to_string()));
        }
        self.managers
            .set_auto_trade(manager_id, enabled)
            .await
            .map_err(AppError::Internal)
    }
}
