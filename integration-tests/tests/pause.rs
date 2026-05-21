use near_workspaces::types::NearToken;
use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

use crate::common::storage::register_for_storage;

#[tokio::test]
#[tracing::instrument]
async fn test_pause_all() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    info!("call pa_pause_feature(ALL) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "pa_pause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method pa_pause_feature restricted by access control."));
    info!("pa_pause_feature: {result:?}");

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
    assert!(!role_is_granted);
    info!("acl_grant_role: {role_is_granted:?}");

    info!("call pa_pause_feature(ALL) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pa_pause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pa_pause_feature: {is_paused:?}");

    info!("call pa_pause_feature(ALL) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pa_pause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .json()?;
    assert!(!is_paused);
    info!("pa_pause_feature: {is_paused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice]");
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
    assert!(result.has_panic("Method is paused"));
    info!("ft_transfer: {result:?}");

    info!("call pa_unpause_feature(ALL) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "pa_unpause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method pa_unpause_feature restricted by access control."));
    info!("pa_unpause_feature: {result:?}");

    info!("call acl_grant_role(PauseManager, alice) [signer=contract]");
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

    info!("call pa_unpause_feature(ALL) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "pa_unpause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("pa_unpause_feature: {is_unpaused:?}");

    info!("call pa_unpause_feature(ALL) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "pa_unpause_feature")
        .args_json(json!({ "key": "ALL" }))
        .transact()
        .await?
        .json()?;
    assert!(!is_unpaused);
    info!("pa_unpause_feature: {is_unpaused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice]");
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
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("ft_transfer: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_pause_minting() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

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

    info!("call pa_pause_feature(minting) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pa_pause_feature")
        .args_json(json!({ "key": "minting" }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pa_pause_feature: {is_paused:?}");

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Method is paused"));
    info!("record_batch: {result:?}");

    info!("call defer_batch([(alice, 10_000)], holding=alice) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": [[context.alice.id(), 10_000]],
            "holding_account_id": context.alice.id(),
        }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Method is paused"));
    info!("defer_batch: {result:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice]");
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
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("ft_transfer (token feature not paused): {result:?}");

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

    info!("call pa_unpause_feature(minting) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "pa_unpause_feature")
        .args_json(json!({ "key": "minting" }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("pa_unpause_feature: {is_unpaused:?}");

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("record_batch: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_pause_token() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

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

    info!("call pa_pause_feature(token) [signer=alice]");
    let is_paused: bool = context
        .alice
        .call(context.sweat.id(), "pa_pause_feature")
        .args_json(json!({ "key": "token" }))
        .transact()
        .await?
        .json()?;
    assert!(is_paused);
    info!("pa_pause_feature: {is_paused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice]");
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
    assert!(result.has_panic("Method is paused"));
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
    assert!(result.has_panic("Method is paused"));
    info!("ft_transfer_call: {result:?}");

    info!("call burn(1000000000000000000) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "burn")
        .args_json(json!({ "amount": "1000000000000000000" }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Method is paused"));
    info!("burn: {result:?}");

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("record_batch (minting feature not paused): {result:?}");

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

    info!("call pa_unpause_feature(token) [signer=alice]");
    let is_unpaused: bool = context
        .alice
        .call(context.sweat.id(), "pa_unpause_feature")
        .args_json(json!({ "key": "token" }))
        .transact()
        .await?
        .json()?;
    assert!(is_unpaused);
    info!("pa_unpause_feature: {is_unpaused:?}");

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice]");
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
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("ft_transfer: {result:?}");

    info!("call burn(1000000000000000000) [signer=alice]");
    let result = context
        .alice
        .call(context.sweat.id(), "burn")
        .args_json(json!({ "amount": "1000000000000000000" }))
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());
    info!("burn: {result:?}");

    Ok(())
}
