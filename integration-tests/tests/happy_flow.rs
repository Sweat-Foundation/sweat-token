use serde_json::json;
use tracing::info;

mod common;
use common::prepare::Context;

#[tokio::test]
#[tracing::instrument]
async fn happy_flow() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("view formula(steps_since_tge=100000, steps=100)");
    let formula: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "100000", "steps": 100 }))
        .await?
        .json()?;
    info!(value = %formula, "formula result");
    assert_eq!(99_999_995_378_125_008_u128, formula.parse::<u128>()?);

    info!("call tge_mint(alice, 100_000_000) [signer=sweat]");
    context
        .sweat
        .call("tge_mint")
        .args_json(json!({ "account_id": context.alice.id(), "amount": "100000000" }))
        .transact()
        .await?
        .into_result()?;

    info!("view ft_balance_of(alice)");
    let balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %balance, "alice balance");
    assert_eq!(100_000_000_u128, balance.parse::<u128>()?);

    info!("call defer_batch([(alice, 1000)], holding=claim) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": [[context.alice.id(), 1000]],
            "holding_account_id": context.claim().id(),
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("done");
    Ok(())
}
