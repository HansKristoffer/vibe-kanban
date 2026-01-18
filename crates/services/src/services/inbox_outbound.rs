use std::env;

use db::models::inbox_item::{InboxItem, InboxSource};
use db::models::project_integrations::ProjectIntegrations;
use db::models::inbox_item::InboxItemStatus;
use sqlx::SqlitePool;

use super::inbox_integrations::{intercom_post_internal_note, linear_post_comment};

fn base_url() -> Option<String> {
    env::var("VK_PUBLIC_BASE_URL")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
}

fn action_link(base: Option<&str>, path: &str) -> String {
    if let Some(base) = base {
        format!("{base}{path}")
    } else {
        path.to_string()
    }
}

fn issue_id_for_item(item: &InboxItem) -> Option<&str> {
    if let Some(id) = item.linear_issue_id.as_deref() {
        return Some(id);
    }
    match item.source {
        InboxSource::Linear => Some(item.source_item_id.as_str()),
        _ => None,
    }
}

fn task_link(base: Option<&str>, item: &InboxItem) -> Option<String> {
    let task_id = item.task_id?;
    Some(action_link(
        base,
        &format!("/projects/{}/tasks/{}", item.project_id, task_id),
    ))
}

pub async fn post_registered_if_needed(
    pool: &SqlitePool,
    integrations: &ProjectIntegrations,
    item: &InboxItem,
) {
    if item.outbound_registered_at.is_some() || !matches!(item.status, InboxItemStatus::Pending) {
        return;
    }

    let base = base_url();
    let accept_path = format!("/api/inbox/action/{}/accept", item.action_token);
    let decline_path = format!("/api/inbox/action/{}/decline", item.action_token);
    let accept_url = action_link(base.as_deref(), &accept_path);
    let decline_url = action_link(base.as_deref(), &decline_path);

    let message = format!(
        "Vibe Kanban: this item is now in the Inbox.\nAccept: {}\nDeny: {}",
        accept_url, decline_url
    );

    match item.source {
        InboxSource::Linear => {
            if let (Some(api_key), Some(issue_id)) =
                (integrations.linear_api_key.as_ref(), issue_id_for_item(item))
            {
                if linear_post_comment(api_key, issue_id, &message).await.is_ok() {
                    let _ = InboxItem::set_outbound_registered(pool, item.id).await;
                }
            }
        }
        InboxSource::Intercom => {
            if let (Some(token), Some(admin_id)) = (
                integrations.intercom_access_token.as_ref(),
                integrations.intercom_admin_id.as_ref(),
            ) {
                if intercom_post_internal_note(token, admin_id, &item.source_item_id, &message)
                    .await
                    .is_ok()
                {
                    let _ = InboxItem::set_outbound_registered(pool, item.id).await;
                }
            }
        }
        _ => {}
    }
}

pub async fn post_started_if_needed(
    pool: &SqlitePool,
    integrations: &ProjectIntegrations,
    item: &InboxItem,
) {
    if item.outbound_started_at.is_some() || !matches!(item.status, InboxItemStatus::Accepted) {
        return;
    }

    let base = base_url();
    let task_link = task_link(base.as_deref(), item);
    let message = match task_link {
        Some(link) => format!("Vibe Kanban: work started. Task: {link}"),
        None => "Vibe Kanban: work started.".to_string(),
    };

    if let (Some(api_key), Some(issue_id)) =
        (integrations.linear_api_key.as_ref(), issue_id_for_item(item))
    {
        if linear_post_comment(api_key, issue_id, &message).await.is_ok() {
            let _ = InboxItem::set_outbound_started(pool, item.id).await;
        }
    }

    if matches!(item.source, InboxSource::Intercom) {
        if let (Some(token), Some(admin_id)) = (
            integrations.intercom_access_token.as_ref(),
            integrations.intercom_admin_id.as_ref(),
        ) {
            if intercom_post_internal_note(token, admin_id, &item.source_item_id, &message)
                .await
                .is_ok()
            {
                let _ = InboxItem::set_outbound_started(pool, item.id).await;
            }
        }
    }
}

pub async fn post_pr_ready_if_needed(
    pool: &SqlitePool,
    integrations: &ProjectIntegrations,
    item: &InboxItem,
    pr_url: &str,
) {
    if item.outbound_pr_created_at.is_some() {
        return;
    }

    let message = format!("Vibe Kanban: PR ready: {}", pr_url);

    if let (Some(api_key), Some(issue_id)) =
        (integrations.linear_api_key.as_ref(), issue_id_for_item(item))
    {
        if linear_post_comment(api_key, issue_id, &message).await.is_ok() {
            let _ = InboxItem::set_outbound_pr_created(pool, item.id).await;
        }
    }

    if matches!(item.source, InboxSource::Intercom) {
        if let (Some(token), Some(admin_id)) = (
            integrations.intercom_access_token.as_ref(),
            integrations.intercom_admin_id.as_ref(),
        ) {
            if intercom_post_internal_note(token, admin_id, &item.source_item_id, &message)
                .await
                .is_ok()
            {
                let _ = InboxItem::set_outbound_pr_created(pool, item.id).await;
            }
        }
    }
}

pub async fn post_pr_merged_if_needed(
    pool: &SqlitePool,
    integrations: &ProjectIntegrations,
    item: &InboxItem,
    pr_url: &str,
) {
    if item.outbound_pr_merged_at.is_some() {
        return;
    }

    let message = format!("Vibe Kanban: PR merged: {}", pr_url);

    if let (Some(api_key), Some(issue_id)) =
        (integrations.linear_api_key.as_ref(), issue_id_for_item(item))
    {
        if linear_post_comment(api_key, issue_id, &message).await.is_ok() {
            let _ = InboxItem::set_outbound_pr_merged(pool, item.id).await;
        }
    }

    if matches!(item.source, InboxSource::Intercom) {
        if let (Some(token), Some(admin_id)) = (
            integrations.intercom_access_token.as_ref(),
            integrations.intercom_admin_id.as_ref(),
        ) {
            if intercom_post_internal_note(token, admin_id, &item.source_item_id, &message)
                .await
                .is_ok()
            {
                let _ = InboxItem::set_outbound_pr_merged(pool, item.id).await;
            }
        }
    }
}
