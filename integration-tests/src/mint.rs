use serde_json::json;

use crate::helpers::{payout, step};
use crate::prepare::prepare_contract;

const TARGET_BALANCE: u128 = 9_999_999_976_902_174_720;
const TARGET_STEPS_SINCE_TGE: u32 = 10_000;
const TAG: &str = "test_mint";

#[tokio::test]
async fn test_mint() -> anyhow::Result<()> {
    let context = prepare_contract(TAG).await?;

    step!(TAG, "view get_steps_since_tge");
    let steps: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    step!(TAG, "  = {}", steps);
    assert_eq!(0_u64, steps.parse::<u64>()?);

    step!(TAG, "view formula(0, {})", TARGET_STEPS_SINCE_TGE);
    let formula: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "0", "steps": TARGET_STEPS_SINCE_TGE }))
        .await?
        .json()?;
    step!(TAG, "  = {}", formula);
    assert_eq!(TARGET_BALANCE, formula.parse::<u128>()?);

    step!(TAG, "call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    let (expected_amount_for_user, expected_fee) = payout(TARGET_BALANCE);
    step!(
        TAG,
        "  expected payout: fee={}, amount_for_user={}",
        expected_fee,
        expected_amount_for_user
    );

    step!(TAG, "view ft_balance_of(oracle)");
    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", oracle_balance);
    assert_eq!(expected_fee, oracle_balance.parse::<u128>()?);

    step!(TAG, "view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", alice_balance);
    assert_eq!(expected_amount_for_user, alice_balance.parse::<u128>()?);

    step!(TAG, "view get_steps_since_tge");
    let steps_after: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    step!(TAG, "  = {}", steps_after);
    assert_eq!(u64::from(TARGET_STEPS_SINCE_TGE), steps_after.parse::<u64>()?);

    step!(TAG, "done");
    Ok(())
}
