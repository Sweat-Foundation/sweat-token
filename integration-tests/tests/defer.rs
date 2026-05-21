use serde_json::json;
use tracing::info;

mod common;
use common::prepare::Context;

const BATCH_SIZE: u32 = 135;
const CLAIM_AMOUNT: u32 = 10_000;

#[tokio::test]
#[tracing::instrument]
async fn test_defer() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("view ft_balance_of(claim)");
    let claim_balance_before: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.claim().id() }))
        .await?
        .json()?;
    info!(value = %claim_balance_before, "claim balance before");
    assert_eq!(0_u128, claim_balance_before.parse::<u128>()?);

    info!(
        batch_size = BATCH_SIZE,
        claim_amount = CLAIM_AMOUNT,
        "view calculate_payout_with_fee_for_batch"
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
    info!(%total_fee, %total_for_user, "payout breakdown");

    let batch: Vec<_> = (0..BATCH_SIZE).map(|_| (context.alice.id(), CLAIM_AMOUNT)).collect();

    info!(
        batch_size = BATCH_SIZE,
        claim_amount = CLAIM_AMOUNT,
        "call defer_batch(holding=claim) [signer=oracle]"
    );
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": batch,
            "holding_account_id": context.claim().id(),
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %alice_balance, "alice balance");
    assert_eq!(0_u128, alice_balance.parse::<u128>()?);

    info!("view ft_balance_of(claim)");
    let claim_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.claim().id() }))
        .await?
        .json()?;
    info!(value = %claim_balance, "claim balance");
    assert_eq!(total_for_user, claim_balance);

    info!("view ft_balance_of(oracle)");
    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    info!(value = %oracle_balance, "oracle balance");
    assert_eq!(total_fee, oracle_balance);

    info!("done");
    Ok(())
}
