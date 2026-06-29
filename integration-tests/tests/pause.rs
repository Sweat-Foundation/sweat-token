use near_workspaces::types::NearToken;
use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

#[tokio::test]
#[tracing::instrument]
async fn test_pause_all() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().with_bob().build().await?;

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

    info!("call acl_grant_role(PauseManager, alice) [signer=contract]");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call acl_grant_role(PauseManager, alice) [signer=contract] — idempotent");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(!role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call pause_features(ALL) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pause_features: {is_paused:?}");

    info!("call pause_features(ALL) [signer=alice] — idempotent");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(!is_paused);
    info!("pause_features: {is_paused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice] — pause check fires before balance check");
    let result = context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob().id(),
            "amount": "1000000000000000000",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'token' is paused"));
    info!("ft_transfer: {result:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'minting' is paused"));
    info!("defer_batch: {result:?}");

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

    info!("call acl_grant_role(UnpauseManager, alice) [signer=contract]");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "UnpauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call unpause_features(ALL) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "unpause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("unpause_features: {is_unpaused:?}");

    info!("call unpause_features(ALL) [signer=alice] — idempotent");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "unpause_features")
        .args_json(json!({ "features": ["token", "minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(!is_unpaused);
    info!("unpause_features: {is_unpaused:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle] — minting works after unpause");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_pause_minting() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("call acl_grant_role(PauseManager, alice) [signer=contract]");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call pause_features(minting) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pause_features: {is_paused:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'minting' is paused"));
    info!("defer_batch: {result:?}");

    info!("call acl_grant_role(UnpauseManager, alice) [signer=contract]");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "UnpauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call unpause_features(minting) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "unpause_features")
        .args_json(json!({ "features": ["minting"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("unpause_features: {is_unpaused:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle] — minting works after unpause");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_pause_token() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().with_bob().build().await?;

    info!("call acl_grant_role(PauseManager, alice) [signer=contract]");
    let role_is_granted: bool = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "PauseManager",
            "account_id": context.alice.id(),
        }))
        .transact()
        .await?
        .json()?;
    assert!(role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call pause_features(token) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pause_features")
        .args_json(json!({ "features": ["token"] }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pause_features: {is_paused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice] — pause check fires before balance check");
    let result = context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob().id(),
            "amount": "1000000000000000000",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'token' is paused"));
    info!("ft_transfer: {result:?}");

    info!("call ft_transfer_call(bob, 1000000000000000000) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": context.bob().id(),
            "amount": "1000000000000000000",
            "msg": "",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'token' is paused"));
    info!("ft_transfer_call: {result:?}");

    info!("call burn(1000000000000000000) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "burn")
        .args_json(json!({ "amount": "1000000000000000000" }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Feature 'token' is paused"));
    info!("burn: {result:?}");

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle] — minting feature not paused, should succeed");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}
