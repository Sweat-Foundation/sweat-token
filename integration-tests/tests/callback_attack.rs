use serde_json::json;
use tracing::info;

mod common;
use common::panic::PanicFinder;
use common::prepare::Context;

#[tokio::test]
#[tracing::instrument]
async fn test_call_on_record_in_callback() -> anyhow::Result<()> {
    let context = Context::builder().with_stub().build().await?;

    info!("view ft_balance_of(alice) [before attack]");
    let balance_before: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %balance_before, "alice balance before");

    info!("alice → stub.exploit_on_record(ft, 1_000_000)");
    let result = context
        .alice
        .call(context.stub().id(), "exploit_on_record")
        .args_json(json!({
            "ft_account_id": context.sweat.id(),
            "amount": "1000000",
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    assert!(
        result.has_panic("Method on_record is private"),
        "expected nested panic \"Method on_record is private\" in receipts"
    );
    info!("nested panic caught as expected");

    info!("view ft_balance_of(alice) [after attack]");
    let balance_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %balance_after, "alice balance after");
    assert_eq!(balance_before, balance_after);

    info!("done");
    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_call_on_record_directly() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("sweat → sweat.on_record(...) [direct call, no preceding promise]");
    let result = context
        .sweat
        .as_account()
        .call(context.sweat.id(), "on_record")
        .args_json(json!({
            "receiver_id": context.alice.id(),
            "amount": "1000000",
            "fee_account_id": context.alice.id(),
            "fee": "2000000",
            "steps_increment": 0,
        }))
        .max_gas()
        .transact()
        .await?
        .into_result();

    assert!(
        result.has_panic("Contract expected a single result on the callback"),
        "expected panic \"Contract expected a single result on the callback\""
    );
    info!("panic caught as expected");

    info!("done");
    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_call_ft_resolve_transfer() -> anyhow::Result<()> {
    let context = Context::builder().with_bob().build().await?;

    info!("alice → sweat.ft_resolve_transfer(...) [private callback]");
    let result = context
        .alice
        .call(context.sweat.id(), "ft_resolve_transfer")
        .args_json(json!({
            "sender_id": context.alice.id(),
            "receiver_id": context.bob().id(),
            "amount": "1000000",
        }))
        .max_gas()
        .transact()
        .await?
        .into_result();

    assert!(
        result.has_panic("Method ft_resolve_transfer is private"),
        "expected panic \"Method ft_resolve_transfer is private\""
    );
    info!("panic caught as expected");

    info!("done");
    Ok(())
}
