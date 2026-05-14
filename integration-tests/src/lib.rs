#![cfg(test)]

mod callback_attack;
mod defer;
mod formula;
mod helpers;
mod mint;
mod panic;
mod prepare;
mod storage;
mod transfer;

use serde_json::json;

use crate::helpers::step;
use crate::prepare::prepare_contract;

// Tests are being resurrected one by one against near-workspaces 0.22.
// Re-enable each module as it's ported off the dead sweat-model + integration-utils deps.
//
// mod callback_attack;
// mod common;
// mod defer;
// mod formula;
// mod interface;
// mod measure;
// mod mint;
// mod transfer;

const TAG: &str = "happy_flow";

#[tokio::test]
async fn happy_flow() -> anyhow::Result<()> {
    let context = prepare_contract(TAG).await?;

    step!(TAG, "view formula(steps_since_tge=100000, steps=100)");
    let formula: String = context
        .sweat
        .view("formula")
        .args_json(json!({ "steps_since_tge": "100000", "steps": 100 }))
        .await?
        .json()?;
    step!(TAG, "  = {}", formula);
    assert_eq!(99_999_995_378_125_008_u128, formula.parse::<u128>()?);

    step!(TAG, "call tge_mint(alice, 100_000_000) [signer=sweat]");
    context
        .sweat
        .call("tge_mint")
        .args_json(json!({ "account_id": context.alice.id(), "amount": "100000000" }))
        .transact()
        .await?
        .into_result()?;

    step!(TAG, "view ft_balance_of(alice)");
    let balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", balance);
    assert_eq!(100_000_000_u128, balance.parse::<u128>()?);

    step!(TAG, "call defer_batch([(alice, 1000)], holding=claim) [signer=oracle]");
    context
        .oracle
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": [[context.alice.id(), 1000]],
            "holding_account_id": context.claim.id(),
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    step!(TAG, "done");
    Ok(())
}
