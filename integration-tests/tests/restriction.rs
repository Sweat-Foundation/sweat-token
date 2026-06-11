use near_workspaces::types::NearToken;
use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context, storage::register_for_storage};

async fn restrict_alice(context: &Context) -> anyhow::Result<()> {
    info!("call acl_grant_role(DenylistManager, sweat) [signer=contract, super-admin]");
    context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "DenylistManager",
            "account_id": context.sweat.id(),
        }))
        .transact()
        .await?
        .into_result()?;

    info!("call set_restricted(alice, true) [signer=contract, DenylistManager]");
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
    let context = Context::builder().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    restrict_alice(&context).await?;

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
    let context = Context::builder().with_bob().build().await?;

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    restrict_alice(&context).await?;

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
    let context = Context::builder().build().await?;

    restrict_alice(&context).await?;

    info!("call burn(1000000000000000000) [signer=alice, restricted]");
    let result = context
        .alice
        .call(context.sweat.id(), "burn")
        .args_json(json!({ "amount": "1000000000000000000" }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic(&format!("The account {} is restricted", context.alice.id())));
    info!("burn: {result:?}");

    Ok(())
}
