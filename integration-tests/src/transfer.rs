use near_workspaces::types::NearToken;
use serde_json::json;

use crate::helpers::step;
use crate::prepare::prepare_contract;
use crate::storage::register_for_storage;

const TAG: &str = "test_transfer";
const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[tokio::test]
async fn test_transfer() -> anyhow::Result<()> {
    let context = prepare_contract(TAG).await?;

    step!(TAG, "call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    step!(
        TAG,
        "call ft_transfer(alice → bob, 9499999991723028480) [bob not registered, expect failure]"
    );
    let res = context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob.id(),
            "amount": "9499999991723028480",
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?
        .into_result();
    assert!(res.is_err(), "ft_transfer to unregistered bob should fail");
    step!(TAG, "  ✓ rejected");

    step!(TAG, "register bob for FT storage");
    register_for_storage(&context.sweat, context.bob.id()).await?;

    step!(TAG, "view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", alice_balance);
    assert_ne!(0_u128, alice_balance.parse::<u128>()?);

    step!(TAG, "call ft_transfer(alice → bob, {}) [transfer all]", alice_balance);
    context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob.id(),
            "amount": alice_balance,
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?
        .into_result()?;

    step!(TAG, "view ft_balance_of(alice)");
    let alice_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", alice_after);
    assert_eq!(0_u128, alice_after.parse::<u128>()?);

    step!(TAG, "view ft_balance_of(bob)");
    let bob_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.bob.id() }))
        .await?
        .json()?;
    step!(TAG, "  = {}", bob_balance);
    assert_eq!(alice_balance, bob_balance);

    step!(TAG, "done");
    Ok(())
}
