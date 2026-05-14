use serde_json::json;

use crate::helpers::step;
use crate::panic::PanicFinder;
use crate::prepare::prepare_contract;

#[tokio::test]
async fn test_call_on_record_in_callback() -> anyhow::Result<()> {
    const TAG: &str = "test_call_on_record_in_callback";
    let context = prepare_contract(TAG).await?;

    step!(TAG, "view ft_balance_of(alice) [before attack]");
    let balance_before: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", balance_before);

    step!(TAG, "alice → stub.exploit_on_record(ft, 1_000_000)");
    let result = context
        .alice
        .call(context.stub.id(), "exploit_on_record")
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
    step!(TAG, "  ✓ nested panic caught");

    step!(TAG, "view ft_balance_of(alice) [after attack]");
    let balance_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", balance_after);
    assert_eq!(balance_before, balance_after);

    step!(TAG, "done");
    Ok(())
}

#[tokio::test]
async fn test_call_on_record_directly() -> anyhow::Result<()> {
    const TAG: &str = "test_call_on_record_directly";
    let context = prepare_contract(TAG).await?;

    step!(TAG, "sweat → sweat.on_record(...) [direct call, no preceding promise]");
    let result = context
        .sweat
        .as_account()
        .call(context.sweat.id(), "on_record")
        .args_json(json!({
            "receiver_id": context.alice.id(),
            "amount": "1000000",
            "fee_account_id": context.alice.id(),
            "fee": "2000000",
        }))
        .max_gas()
        .transact()
        .await?
        .into_result();

    assert!(
        result.has_panic("Contract expected a single result on the callback"),
        "expected panic \"Contract expected a single result on the callback\""
    );
    step!(TAG, "  ✓ panic caught");

    step!(TAG, "done");
    Ok(())
}

#[tokio::test]
async fn test_call_ft_resolve_transfer() -> anyhow::Result<()> {
    const TAG: &str = "test_call_ft_resolve_transfer";
    let context = prepare_contract(TAG).await?;

    step!(TAG, "alice → sweat.ft_resolve_transfer(...) [private callback]");
    let result = context
        .alice
        .call(context.sweat.id(), "ft_resolve_transfer")
        .args_json(json!({
            "sender_id": context.alice.id(),
            "receiver_id": context.bob.id(),
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
    step!(TAG, "  ✓ panic caught");

    step!(TAG, "done");
    Ok(())
}
