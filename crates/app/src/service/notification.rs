use std::sync::Arc;

use uuid::Uuid;

use lumos_domain::port::notification::{Notification, NotificationProvider};

use crate::repo::order_plan::OrderPlan;

pub struct NotificationService {
    provider: Arc<dyn NotificationProvider>,
}

impl NotificationService {
    pub fn new(provider: Arc<dyn NotificationProvider>) -> Self {
        Self { provider }
    }

    pub async fn notify_risk_rejected(&self, manager_id: Uuid, plan: &OrderPlan) {
        let reason = plan.risk_reject_reason.as_deref().unwrap_or("알 수 없음");
        let body = format!(
            "매니저 {manager_id}\n심볼: {}\n사유: {reason}",
            plan.symbol_id
        );
        let n = Notification::warning("리스크 게이트 거절", body);
        if let Err(e) = self.provider.send(n).await {
            tracing::warn!("notification send failed: {e:?}");
        }
    }

    pub async fn notify_trade_filled(
        &self,
        manager_id: Uuid,
        symbol_id: Uuid,
        side: &str,
        amount: &str,
    ) {
        let body = format!("매니저 {manager_id}\n{side} {symbol_id} — {amount}");
        let n = Notification::info("주문 체결", body);
        if let Err(e) = self.provider.send(n).await {
            tracing::warn!("notification send failed: {e:?}");
        }
    }
}
