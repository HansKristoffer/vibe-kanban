use std::time::Duration;

use db::{
    DBService,
    models::{
        inbox_source_cursor::{InboxSourceCursor, InboxSourceCursorType},
        project_integrations::ProjectIntegrations,
    },
};
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Background poller for Modjo (and optional Linear backfill).
pub struct InboxPollerService {
    db: DBService,
    poll_interval: Duration,
}

impl InboxPollerService {
    pub async fn spawn(db: DBService) -> tokio::task::JoinHandle<()> {
        let service = Self {
            db,
            poll_interval: Duration::from_secs(120),
        };
        tokio::spawn(async move {
            service.start().await;
        })
    }

    async fn start(&self) {
        info!(
            "Starting Inbox poller service with interval {:?}",
            self.poll_interval
        );
        let mut interval = interval(self.poll_interval);
        loop {
            interval.tick().await;
            if let Err(err) = self.poll_once().await {
                warn!("Inbox poller failed: {}", err);
            }
        }
    }

    async fn poll_once(&self) -> Result<(), sqlx::Error> {
        let integrations = ProjectIntegrations::find_all(&self.db.pool).await?;
        for integration in integrations {
            if integration.modjo_api_key.is_none() {
                continue;
            }

            debug!(
                "Polling Modjo for project {} (cursor tracking only; ingestion TODO)",
                integration.project_id
            );

            // Placeholder: create/update cursor row to record poll time.
            // Actual Modjo fetch + ingestion will be implemented once API details are finalized.
            let _ = InboxSourceCursor::upsert(
                &self.db.pool,
                integration.project_id,
                InboxSourceCursorType::Modjo,
                None,
            )
            .await?;
        }

        Ok(())
    }
}
