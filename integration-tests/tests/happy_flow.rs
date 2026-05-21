use serde_json::json;
use tracing::info;

mod common;
use common::helpers::payout;
use common::prepare::Context;

#[tokio::test]
#[tracing::instrument]
async fn happy_flow() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("view formula(steps_since_tge=0, steps=100)");
    let formula: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "0", "steps": 100 }))
        .await?
        .json()?;
    info!(value = %formula, "formula result");
    let minted = formula.parse::<u128>()?;

    info!("call record_batch([(alice, 100)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 100]] }))
        .transact()
        .await?
        .into_result()?;

    let (expected_amount_for_user, _expected_fee) = payout(minted);

    info!("view ft_balance_of(alice)");
    let balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %balance, "alice balance");
    assert_eq!(expected_amount_for_user, balance.parse::<u128>()?);

    info!("call defer_batch([(alice, 1000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 1000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("done");
    Ok(())
}
