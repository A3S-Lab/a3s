use std::path::{Path, PathBuf};

use super::*;

async fn write_control_request(directory: &Path, request: &AgentControlProtocolRequest) -> PathBuf {
    let queue = directory.join(CONTROL_REQUEST_DIRECTORY);
    tokio::fs::create_dir_all(&queue).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&queue, std::fs::Permissions::from_mode(0o700))
            .await
            .unwrap();
    }
    let path = queue.join(format!("control-{}.json", request.request_id));
    tokio::fs::write(&path, serde_json::to_vec(request).unwrap())
        .await
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .await
            .unwrap();
    }
    path
}

#[test]
fn control_grants_are_stable_scoped_and_share_one_decision_token() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let activity_id = publisher.instance_id().to_string();
    let actions = [
        AgentControlActionKind::ApproveOnce,
        AgentControlActionKind::ApproveAlways,
        AgentControlActionKind::Deny,
    ];
    let first = publisher.reconcile_control_grants(
        [AgentControlGrantSpec::new(
            activity_id.clone(),
            "tool-a",
            actions,
        )],
        1_000,
    );
    let first = &first[&activity_id];
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|action| action.token == first[0].token));
    assert!(first
        .iter()
        .all(|action| action.target_instance_id == activity_id));

    let second = publisher.reconcile_control_grants(
        [AgentControlGrantSpec::new(
            activity_id.clone(),
            "tool-a",
            actions,
        )],
        2_000,
    );
    assert_eq!(second[&activity_id][0].token, first[0].token);
    assert!(second[&activity_id][0].expires_at_ms > first[0].expires_at_ms);

    let replacement = publisher.reconcile_control_grants(
        [AgentControlGrantSpec::new(
            activity_id.clone(),
            "tool-b",
            actions,
        )],
        2_100,
    );
    assert_ne!(replacement[&activity_id][0].token, first[0].token);
}

#[tokio::test]
async fn earliest_valid_control_consumes_all_alternatives_and_replay_fails() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let now = epoch_ms();
    let activity_id = publisher.instance_id().to_string();
    let actions = publisher.reconcile_control_grants(
        [AgentControlGrantSpec::new(
            activity_id.clone(),
            "tool-a",
            [
                AgentControlActionKind::ApproveOnce,
                AgentControlActionKind::Deny,
            ],
        )],
        now,
    );
    let descriptor = &actions[&activity_id][0];
    let request = |request_id: &str,
                   action: AgentControlActionKind,
                   created_at_ms: u64|
     -> AgentControlProtocolRequest {
        AgentControlProtocolRequest {
            schema: CONTROL_REQUEST_SCHEMA.to_string(),
            request_id: request_id.to_string(),
            target_instance_id: activity_id.clone(),
            activity_id: activity_id.clone(),
            action,
            message: None,
            token: descriptor.token.clone(),
            created_at_ms,
            expires_at_ms: descriptor.expires_at_ms,
        }
    };
    write_control_request(
        temp.path(),
        &request(
            "decision-later",
            AgentControlActionKind::Deny,
            now.saturating_add(1),
        ),
    )
    .await;
    write_control_request(
        temp.path(),
        &request("decision-first", AgentControlActionKind::ApproveOnce, now),
    )
    .await;

    let accepted = publisher
        .consume_control_requests(now.saturating_add(2))
        .await
        .unwrap();
    assert_eq!(
        accepted,
        vec![AgentControlRequest {
            activity_id: activity_id.clone(),
            context: "tool-a".to_string(),
            action: AgentControlActionKind::ApproveOnce,
            message: None,
        }]
    );
    assert!(
        std::fs::read_dir(temp.path().join(CONTROL_REQUEST_DIRECTORY))
            .unwrap()
            .next()
            .is_none()
    );

    write_control_request(
        temp.path(),
        &request(
            "decision-replay",
            AgentControlActionKind::ApproveOnce,
            now.saturating_add(3),
        ),
    )
    .await;
    assert!(publisher
        .consume_control_requests(now.saturating_add(3))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn reply_control_carries_one_bounded_message_to_the_owning_tui() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let now = epoch_ms();
    let activity_id = publisher.instance_id().to_string();
    let actions = publisher.reconcile_control_grants(
        [AgentControlGrantSpec::new(
            activity_id.clone(),
            "active-turn",
            [AgentControlActionKind::Reply],
        )],
        now,
    );
    let descriptor = &actions[&activity_id][0];
    let request = AgentControlProtocolRequest {
        schema: CONTROL_REQUEST_SCHEMA.to_string(),
        request_id: "reply-first".to_string(),
        target_instance_id: activity_id.clone(),
        activity_id: activity_id.clone(),
        action: AgentControlActionKind::Reply,
        message: Some("Use the safer migration path.".to_string()),
        token: descriptor.token.clone(),
        created_at_ms: now,
        expires_at_ms: descriptor.expires_at_ms,
    };
    write_control_request(temp.path(), &request).await;

    assert_eq!(
        publisher.consume_control_requests(now).await.unwrap(),
        vec![AgentControlRequest {
            activity_id,
            context: "active-turn".to_string(),
            action: AgentControlActionKind::Reply,
            message: Some("Use the safer migration path.".to_string()),
        }]
    );
}
