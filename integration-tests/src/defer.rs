use serde_json::json;

use crate::helpers::step;
use crate::prepare::prepare_contract;

const TAG: &str = "test_defer";
const BATCH_SIZE: u32 = 135;
const CLAIM_AMOUNT: u32 = 10_000;

#[tokio::test]
async fn test_defer() -> anyhow::Result<()> {
    let context = prepare_contract(TAG).await?;

    step!(TAG, "view ft_balance_of(claim)");
    let claim_balance_before: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.claim.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", claim_balance_before);
    assert_eq!(0_u128, claim_balance_before.parse::<u128>()?);

    step!(
        TAG,
        "view calculate_payout_with_fee_for_batch(batch_size={BATCH_SIZE}, claim_amount={CLAIM_AMOUNT})"
    );
    let (total_fee, total_for_user): (String, String) = context
        .sweat
        .view("calculate_payout_with_fee_for_batch")
        .args_json(json!({
            "batch_size": BATCH_SIZE,
            "claim_amount": CLAIM_AMOUNT,
        }))
        .await?
        .json()?;
    step!(TAG, "  total_fee={total_fee}, total_for_user={total_for_user}");

    let batch: Vec<_> = (0..BATCH_SIZE)
        .map(|_| (context.alice.id(), CLAIM_AMOUNT))
        .collect();

    step!(
        TAG,
        "call defer_batch([(alice, {CLAIM_AMOUNT})] × {BATCH_SIZE}, holding=claim) [signer=oracle]"
    );
    context
        .oracle
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": batch,
            "holding_account_id": context.claim.id(),
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    step!(TAG, "view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", alice_balance);
    assert_eq!(0_u128, alice_balance.parse::<u128>()?);

    step!(TAG, "view ft_balance_of(claim)");
    let claim_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.claim.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", claim_balance);
    assert_eq!(total_for_user, claim_balance);

    step!(TAG, "view ft_balance_of(oracle)");
    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", oracle_balance);
    assert_eq!(total_fee, oracle_balance);

    step!(TAG, "done");
    Ok(())
}
