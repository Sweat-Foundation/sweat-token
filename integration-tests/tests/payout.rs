//! Drift guard for `helpers::payout()`.
//!
//! `helpers::payout()` mirrors `Payout::from(u128)` in `contract/src/api.rs`.
//! Nothing forces them to stay in sync. This test asks the deployed contract
//! to compute a single-row payout via `calculate_payout_with_fee_for_batch`
//! (with batch_size=1, internally `Payout::from(formula(0, STEPS))`) and
//! compares to the locally-computed split. Divergence — a fee-percentage
//! change, a rounding-rule change, a swapped tuple order — fails here first.

use serde_json::json;
use tracing::info;

mod common;
use common::helpers::payout;
use common::prepare::Context;

const STEPS: u32 = 10_000;

#[tokio::test]
#[tracing::instrument]
async fn payout_helper_matches_contract() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!(steps = STEPS, "view formula(0, STEPS)");
    let minted: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "0", "steps": STEPS }))
        .await?
        .json()?;
    let value: u128 = minted.parse()?;
    info!(value, "minted amount for one row");

    info!("view calculate_payout_with_fee_for_batch(batch_size=1, claim_amount=STEPS)");
    let (contract_fee, contract_for_user): (String, String) = context
        .sweat
        .view("calculate_payout_with_fee_for_batch")
        .args_json(json!({ "batch_size": 1, "claim_amount": STEPS }))
        .await?
        .json()?;
    let contract_fee: u128 = contract_fee.parse()?;
    let contract_for_user: u128 = contract_for_user.parse()?;
    info!(contract_fee, contract_for_user, "contract payout split");

    let (local_for_user, local_fee) = payout(value);
    info!(local_fee, local_for_user, "helper payout split");

    assert_eq!(
        local_fee, contract_fee,
        "fee diverged: helper says {local_fee}, contract says {contract_fee} (value={value})"
    );
    assert_eq!(
        local_for_user, contract_for_user,
        "amount_for_user diverged: helper says {local_for_user}, contract says {contract_for_user} (value={value})"
    );

    info!("payout helper matches contract Payout::from");
    Ok(())
}
