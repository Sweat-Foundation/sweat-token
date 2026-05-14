use serde_json::json;
use tracing::info;

mod common;
use common::helpers::payout;
use common::prepare::Context;

const TARGET_BALANCE: u128 = 9_999_999_976_902_174_720;
const TARGET_STEPS_SINCE_TGE: u32 = 10_000;

#[tokio::test]
#[tracing::instrument]
async fn test_mint() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().build().await?;

    info!("view get_steps_since_tge");
    let steps: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    info!(value = %steps, "steps_since_tge");
    assert_eq!(0_u64, steps.parse::<u64>()?);

    info!(target_steps = TARGET_STEPS_SINCE_TGE, "view formula(0, target_steps)");
    let formula: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "0", "steps": TARGET_STEPS_SINCE_TGE }))
        .await?
        .json()?;
    info!(value = %formula, "formula result");
    assert_eq!(TARGET_BALANCE, formula.parse::<u128>()?);

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    let (expected_amount_for_user, expected_fee) = payout(TARGET_BALANCE);
    info!(fee = expected_fee, for_user = expected_amount_for_user, "expected payout");

    info!("view ft_balance_of(oracle)");
    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    info!(value = %oracle_balance, "oracle balance");
    assert_eq!(expected_fee, oracle_balance.parse::<u128>()?);

    info!("view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %alice_balance, "alice balance");
    assert_eq!(expected_amount_for_user, alice_balance.parse::<u128>()?);

    info!("view get_steps_since_tge");
    let steps_after: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    info!(value = %steps_after, "steps_since_tge");
    assert_eq!(u64::from(TARGET_STEPS_SINCE_TGE), steps_after.parse::<u64>()?);

    info!("done");
    Ok(())
}
