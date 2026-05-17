use std::sync::Arc;
use uuid::Uuid;

use lumos_domain::model::manager::Manager;
use lumos_domain::model::risk::RiskPolicy;

use crate::error::{AppError, AppResult};
use crate::repo::broker_connection::BrokerConnectionRepository;
use crate::repo::manager::{CreateManagerInput, ManagerRepository, RiskPolicyRepository};

pub struct ManagerService {
    managers: Arc<dyn ManagerRepository>,
    policies: Arc<dyn RiskPolicyRepository>,
    broker_connections: Option<Arc<dyn BrokerConnectionRepository>>,
}

impl ManagerService {
    pub fn new(
        managers: Arc<dyn ManagerRepository>,
        policies: Arc<dyn RiskPolicyRepository>,
    ) -> Self {
        Self {
            managers,
            policies,
            broker_connections: None,
        }
    }

    pub fn with_broker_connection_repo(
        mut self,
        repo: Arc<dyn BrokerConnectionRepository>,
    ) -> Self {
        self.broker_connections = Some(repo);
        self
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
        // broker_connection 존재 및 소유자 검증
        if let Some(bc_repo) = &self.broker_connections {
            let conn = bc_repo
                .find_by_id(input.broker_connection_id)
                .await
                .map_err(AppError::Internal)?
                .ok_or_else(|| {
                    AppError::NotFound(format!("broker_connection {}", input.broker_connection_id))
                })?;

            if conn.user_id != input.user_id {
                return Err(AppError::Forbidden(
                    "broker_connection does not belong to this user".to_string(),
                ));
            }
        }

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
