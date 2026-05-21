use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

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
        "call defer_batch [signer=oracle]"
    );
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": batch }))
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

/// `defer_batch` must refuse to run until the super admin has configured a
/// holding account — there is no implicit default to fall back to.
#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_panics_when_holding_account_unset() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().build().await?;

    info!("view get_holding_account_id");
    let holding: Option<String> = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, None);
    info!("holding account is unset");

    info!("call defer_batch([(alice, {CLAIM_AMOUNT})]) [signer=oracle] — holding account unset");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), CLAIM_AMOUNT]] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Holding account is not set"));
    info!("defer_batch: {result:?}");

    Ok(())
}

/// The super admin can re-point the holding account via `set_holding_account_id`,
/// and `defer_batch` immediately stages deferred mints through the new account.
#[tokio::test]
#[tracing::instrument]
async fn test_set_holding_account_id_updates_defer_target() -> anyhow::Result<()> {
    let context = Context::builder()
        .with_oracle()
        .with_claim()
        .with_stub()
        .build()
        .await?;

    info!("view get_holding_account_id [baked in at init]");
    let holding: Option<String> = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, Some(context.claim().id().to_string()));
    info!(?holding, "holding account starts as claim");

    info!("call set_holding_account_id(stub) [signer=contract, super-admin]");
    context
        .sweat
        .call("set_holding_account_id")
        .args_json(json!({ "account_id": context.stub().id() }))
        .transact()
        .await?
        .into_result()?;

    info!("view get_holding_account_id [after update]");
    let holding: Option<String> = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, Some(context.stub().id().to_string()));
    info!(?holding, "holding account updated to stub");

    info!("view formula(0, {CLAIM_AMOUNT}) — raw deferred amount");
    let minted_raw: String = context
        .sweat
        .view("formula")
        .args_json(json!({
            "steps_since_tge": "0",
            "steps": CLAIM_AMOUNT,
        }))
        .await?
        .json()?;
    // Mirror `Payout::from`: fee is 5% rounded up, the rest goes to the holder.
    let minted: u128 = minted_raw.parse()?;
    let total_fee = (minted * 5).div_ceil(100);
    let total_for_user = minted - total_fee;
    info!(%minted_raw, total_fee, total_for_user, "expected payout breakdown");

    info!("call defer_batch([(alice, {CLAIM_AMOUNT})]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), CLAIM_AMOUNT]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("view ft_balance_of(stub) [updated holding account]");
    let stub_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.stub().id() }))
        .await?
        .json()?;
    assert_eq!(
        stub_balance.parse::<u128>()?,
        total_for_user,
        "deferred mint must follow the updated holding account"
    );
    info!(value = %stub_balance, "stub received the deferred amount");

    info!("view ft_balance_of(claim) [former holding account]");
    let claim_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.claim().id() }))
        .await?
        .json()?;
    assert_eq!(claim_balance, "0", "former holding account must receive nothing");
    info!(value = %claim_balance, "claim received nothing");

    info!("done");
    Ok(())
}

/// `set_holding_account_id` is super-admin-only (`#[private]`); a regular account
/// cannot re-point the holding account.
#[tokio::test]
#[tracing::instrument]
async fn test_set_holding_account_id_rejects_non_admin() -> anyhow::Result<()> {
    let context = Context::builder().with_claim().build().await?;

    info!("call set_holding_account_id(alice) [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "set_holding_account_id")
        .args_json(json!({ "account_id": context.alice.id() }))
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Method set_holding_account_id is private"));
    info!("set_holding_account_id: {result:?}");

    info!("view get_holding_account_id [unchanged]");
    let holding: Option<String> = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, Some(context.claim().id().to_string()));
    info!(?holding, "holding account unchanged");

    Ok(())
}
