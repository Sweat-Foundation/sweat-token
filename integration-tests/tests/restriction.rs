use near_workspaces::types::NearToken;
use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context, storage::register_for_storage};

/// Mint tokens to alice, then add her to the contract denylist.
async fn mint_and_restrict(context: &Context) -> anyhow::Result<()> {
    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    info!("call set_restricted(alice, true) [signer=contract]");
    context
        .sweat
        .call("set_restricted")
        .args_json(json!({
            "account_id": context.alice.id(),
            "is_restricted": true,
        }))
        .transact()
        .await?
        .into_result()?;

    let is_restricted: bool = context
        .sweat
        .view("is_restricted")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert!(is_restricted);
    info!("alice is restricted");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_restricted_cannot_ft_transfer() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    mint_and_restrict(&context).await?;

    info!("call ft_transfer(bob, 1000000000000000000) [signer=alice, restricted]");
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
    assert!(result.has_panic(&format!("The account {} is restricted", context.alice.id())));
    info!("ft_transfer: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_restricted_cannot_ft_transfer_call() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    mint_and_restrict(&context).await?;

    info!("call ft_transfer_call(bob, 1000000000000000000) [signer=alice, restricted]");
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
    assert!(result.has_panic(&format!("The account {} is restricted", context.alice.id())));
    info!("ft_transfer_call: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_restricted_cannot_burn() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().build().await?;

    mint_and_restrict(&context).await?;

    info!("call burn(1000000000000000000) [signer=alice, restricted]");
    let result = context
        .alice
        .call(context.sweat.id(), "burn")
        .args_json(json!({ "amount": "1000000000000000000" }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic(&format!("The account {} is restricted", context.alice.id())));
    info!("burn: {result:?}");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_restricted_cannot_storage_unregister() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().build().await?;

    mint_and_restrict(&context).await?;

    info!("view ft_balance_of(alice) before storage_unregister");
    let balance_before: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert_ne!(balance_before, "0");
    info!(value = %balance_before, "alice balance before");

    info!("call storage_unregister(force=true) [signer=alice, restricted]");
    let result = context
        .alice
        .call(context.sweat.id(), "storage_unregister")
        .args_json(json!({ "force": true }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic(&format!("The account {} is restricted", context.alice.id())));
    info!("storage_unregister: {result:?}");

    info!("view ft_balance_of(alice) after storage_unregister");
    let balance_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert_eq!(
        balance_before, balance_after,
        "restricted account's tokens must be preserved"
    );
    info!(value = %balance_after, "alice balance after");

    Ok(())
}
