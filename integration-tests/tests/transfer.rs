use near_workspaces::types::NearToken;
use serde_json::json;
use tracing::info;

mod common;
use common::prepare::Context;
use common::storage::register_for_storage;

const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[tokio::test]
#[tracing::instrument]
async fn test_transfer() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_bob().build().await?;

    info!("call record_batch([(alice, 10_000)]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .transact()
        .await?
        .into_result()?;

    info!("call ft_transfer(alice → bob, 9499999991723028480) [bob not registered, expect failure]");
    let res = context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob().id(),
            "amount": "9499999991723028480",
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?
        .into_result();
    assert!(res.is_err(), "ft_transfer to unregistered bob should fail");
    info!("rejected as expected");

    info!("register bob for FT storage");
    register_for_storage(&context.sweat, context.bob().id()).await?;

    info!("view ft_balance_of(alice)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %alice_balance, "alice balance");
    assert_ne!(0_u128, alice_balance.parse::<u128>()?);

    info!(amount = %alice_balance, "call ft_transfer(alice → bob) [transfer all]");
    context
        .alice
        .call(context.sweat.id(), "ft_transfer")
        .args_json(json!({
            "receiver_id": context.bob().id(),
            "amount": alice_balance,
        }))
        .deposit(ONE_YOCTO)
        .transact()
        .await?
        .into_result()?;

    info!("view ft_balance_of(alice)");
    let alice_after: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %alice_after, "alice balance");
    assert_eq!(0_u128, alice_after.parse::<u128>()?);

    info!("view ft_balance_of(bob)");
    let bob_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.bob().id() }))
        .await?
        .json()?;
    info!(value = %bob_balance, "bob balance");
    assert_eq!(alice_balance, bob_balance);

    info!("done");
    Ok(())
}
