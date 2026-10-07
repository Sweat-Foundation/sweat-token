use serde_json::{json, Value};
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

/// `batch_ft_transfer` applies every transfer of a batch or none of them: a
/// failing entry reverts the entries before it.
#[tokio::test]
#[tracing::instrument]
async fn test_batch_ft_transfer() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().with_bob().build().await?;

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle] — mints the fee to oracle");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    let oracle_balance: u128 = oracle_balance.parse()?;
    assert!(oracle_balance > 0);
    info!("ft_balance_of(oracle): {oracle_balance}");

    info!("call batch_ft_transfer [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "batch_ft_transfer")
        .args_json(json!({ "transfers": [[context.oracle().id(), context.alice.id(), "1"]] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method batch_ft_transfer"));
    info!("batch_ft_transfer: {result:?}");

    info!("call batch_ft_transfer [signer=oracle] — second entry targets unregistered bob");
    let result = context
        .oracle()
        .call(context.sweat.id(), "batch_ft_transfer")
        .args_json(json!({ "transfers": [
            [context.oracle().id(), context.alice.id(), "100"],
            [context.alice.id(), context.bob().id(), "50"],
        ] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic(&format!("The account {} is not registered", context.bob().id())));
    info!("batch_ft_transfer: {result:?}");

    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert_eq!(alice_balance, "0", "the failed batch must not move any tokens");

    info!("call batch_ft_transfer [signer=oracle]");
    let result = context
        .oracle()
        .call(context.sweat.id(), "batch_ft_transfer")
        .args_json(json!({ "transfers": [
            [context.oracle().id(), context.alice.id(), "100"],
            [context.alice.id(), context.claim().id(), "40"],
        ] }))
        .transact()
        .await?
        .into_result()?;
    info!("batch_ft_transfer: {result:?}");

    let oracle_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    assert_eq!(oracle_after.parse::<u128>()?, oracle_balance - 100);

    let alice_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert_eq!(alice_after, "60");

    Ok(())
}

/// `batch_storage_unregister` removes only zero-balance accounts and keeps the
/// released storage deposit on the contract instead of refunding it.
#[tokio::test]
#[tracing::instrument]
async fn test_batch_storage_unregister() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().with_bob().build().await?;

    info!("call defer_batch([(alice, 10_000)]) [signer=oracle] — gives oracle a positive balance");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    // bob, not alice, makes the unauthorized call: its gas refund would land on
    // alice after the balance snapshot below and break the NEAR comparison.
    info!("call batch_storage_unregister [signer=bob, unauthorized]");
    let result = context
        .bob()
        .call(context.sweat.id(), "batch_storage_unregister")
        .args_json(json!({ "account_ids": [context.alice.id()] }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method batch_storage_unregister"));
    info!("batch_storage_unregister: {result:?}");

    let sweat_near_before = context.sweat.as_account().view_account().await?.balance;
    let alice_near_before = context.alice.view_account().await?.balance;

    info!("call batch_storage_unregister([alice, oracle, bob]) [signer=oracle]");
    let removed: Vec<String> = context
        .oracle()
        .call(context.sweat.id(), "batch_storage_unregister")
        .args_json(json!({ "account_ids": [context.alice.id(), context.oracle().id(), context.bob().id()] }))
        .transact()
        .await?
        .json()?;
    assert_eq!(removed, vec![context.alice.id().to_string()]);
    info!("batch_storage_unregister: {removed:?}");

    let alice_storage: Option<Value> = context
        .sweat
        .view("storage_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert!(alice_storage.is_none(), "alice must be unregistered");

    let oracle_storage: Option<Value> = context
        .sweat
        .view("storage_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    assert!(oracle_storage.is_some(), "oracle holds SWEAT and must stay registered");

    let sweat_near_after = context.sweat.as_account().view_account().await?.balance;
    let alice_near_after = context.alice.view_account().await?.balance;
    assert_eq!(alice_near_after, alice_near_before, "no NEAR is sent to the removed account");
    assert!(
        sweat_near_after >= sweat_near_before,
        "the storage deposit must stay on the contract: before {sweat_near_before}, after {sweat_near_after}"
    );

    Ok(())
}
