use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

#[tokio::test]
#[tracing::instrument]
async fn test_acl_grant_role() -> anyhow::Result<()> {
    let context = Context::builder().with_bob().build().await?;

    info!("call acl_grant_role(Oracle, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "Oracle",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("view acl_has_role(Oracle, alice)");
    let has_role: bool = context
        .sweat
        .view("acl_has_role")
        .args_json(json!({
            "role": "Oracle",
            "account_id": context.alice.id(),
        }))
        .await?
        .json()?;
    assert!(has_role);
    info!("acl_has_role: {has_role:?}");

    info!("call acl_grant_role(Oracle, bob) [signer=alice, unauthorized]");
    let granted: Option<bool> = context
        .alice
        .call(context.sweat.id(), "acl_grant_role")
        .args_json(json!({
            "role": "Oracle",
            "account_id": context.bob().id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, None);
    info!("acl_grant_role: {granted:?}");

    info!("view acl_has_role(Oracle, bob)");
    let has_role: bool = context
        .sweat
        .view("acl_has_role")
        .args_json(json!({
            "role": "Oracle",
            "account_id": context.bob().id(),
        }))
        .await?
        .json()?;
    assert!(!has_role);
    info!("acl_has_role: {has_role:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_acl_defer_batch() -> anyhow::Result<()> {
    let context = Context::builder().with_claim().build().await?;

    info!("call defer_batch([(alice, 10_000)]) [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method defer_batch restricted by access control."));
    info!("defer_batch: {result:?}");

    info!("call acl_grant_role(Oracle, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "Oracle",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=alice, authorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("defer_batch: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_acl_set_restricted() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("call set_restricted(alice, true) [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "set_restricted")
        .args_json(json!({
            "account_id": context.alice.id(),
            "is_restricted": true,
        }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method set_restricted restricted by access control."));
    info!("set_restricted: {result:?}");

    info!("call acl_grant_role(DenylistManager, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "DenylistManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("call set_restricted(alice, true) [signer=alice, authorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "set_restricted")
        .args_json(json!({
            "account_id": context.alice.id(),
            "is_restricted": true,
        }))
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("set_restricted: {result:?}");

    info!("view is_restricted(alice)");
    let is_restricted: bool = context
        .sweat
        .view("is_restricted")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert!(is_restricted);
    info!("is_restricted: {is_restricted:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_acl_pause_feature() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("call pause_features(ALL) [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method pause_features restricted by access control."));
    info!("pause_features: {result:?}");

    info!("call acl_grant_role(PauseManager, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("call pause_features(ALL) [signer=alice, authorized]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pause_features: {is_paused:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_acl_unpause_feature() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("call unpause_features(ALL) [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "unpause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method unpause_features restricted by access control."));
    info!("unpause_features: {result:?}");

    info!("call acl_grant_role(PauseManager, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("call acl_grant_role(UnpauseManager, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "UnpauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));
    info!("acl_grant_role: {granted:?}");

    info!("call pause_features(ALL) [signer=alice, authorized]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pause_features: {is_paused:?}");

    info!("call unpause_features(ALL) [signer=alice, authorized]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "unpause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("unpause_features: {is_unpaused:?}");

    Ok(())
}
