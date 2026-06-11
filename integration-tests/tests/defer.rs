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

#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_skips_denylisted_users() -> anyhow::Result<()> {
    // alice is denylisted, bob is not. A single defer_batch records both. The
    // loop in defer_batch must skip alice (nothing recorded for her to claim)
    // while still recording bob. bob is the positive control proving the batch
    // isn't simply rejected wholesale.
    let context = Context::builder().with_oracle().with_claim().with_bob().build().await?;

    info!("call acl_grant_role(DenylistManager, sweat) [signer=contract, super-admin]");
    context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({
            "role": "DenylistManager",
            "account_id": context.sweat.id(),
        }))
        .transact()
        .await?
        .into_result()?;

    info!("call set_restricted(alice, true) [signer=contract, DenylistManager]");
    context
        .sweat
        .call("set_restricted")
        .args_json(json!({
            "account_id": context.alice.id(),
            "is_restricted": true,
        }))
        .transact()
        .await?
        .into_result()?;

    info!("call defer_batch([(alice, {CLAIM_AMOUNT}), (bob, {CLAIM_AMOUNT})]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({
            "steps_batch": [
                [context.alice.id(), CLAIM_AMOUNT],
                [context.bob().id(), CLAIM_AMOUNT],
            ],
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("view get_claimable_balance_for_account(alice) [denylisted — expect nothing]");
    let alice_claimable: String = context
        .claim()
        .view("get_claimable_balance_for_account")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    info!(value = %alice_claimable, "alice claimable");
    assert_eq!(
        alice_claimable.parse::<u128>()?,
        0,
        "denylisted user must have nothing recorded to claim"
    );

    info!("view get_claimable_balance_for_account(bob) [not denylisted — expect a balance]");
    let bob_claimable: String = context
        .claim()
        .view("get_claimable_balance_for_account")
        .args_json(json!({ "account_id": context.bob().id() }))
        .await?
        .json()?;
    info!(value = %bob_claimable, "bob claimable");
    assert!(
        bob_claimable.parse::<u128>()? > 0,
        "non-denylisted user must have a claimable balance recorded"
    );

    info!("done");
    Ok(())
}

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
    let holding: String = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, context.claim().id().to_string());
    info!(%holding, "holding account starts as claim");

    info!("call set_holding_account_id(stub) [signer=contract, super-admin]");
    context
        .sweat
        .call("set_holding_account_id")
        .args_json(json!({ "account_id": context.stub().id() }))
        .transact()
        .await?
        .into_result()?;

    info!("view get_holding_account_id [after update]");
    let holding: String = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, context.stub().id().to_string());
    info!(%holding, "holding account updated to stub");

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
    let holding: String = context.sweat.view("get_holding_account_id").await?.json()?;
    assert_eq!(holding, context.claim().id().to_string());
    info!(%holding, "holding account unchanged");

    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_rejects_empty_batch() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("call defer_batch([]) [signer=oracle] — empty batch must be rejected");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Empty steps batch"));
    info!("defer_batch: {result:?}");

    info!("done");
    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_rejects_zero_step_count() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("call defer_batch([(alice, 0)]) [signer=oracle] — zero step count must be rejected");
    let result = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 0]] }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Step count must not be zero"));
    info!("defer_batch: {result:?}");

    info!("view get_steps_since_tge [unchanged after rejected batch]");
    let steps: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(
        steps.parse::<u64>()?,
        0,
        "a rejected batch must not advance the steps counter"
    );
    info!(value = %steps, "steps unchanged");

    info!("done");
    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_advances_steps_on_success() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("view get_steps_since_tge [before]");
    let steps_before: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(steps_before.parse::<u64>()?, 0);
    info!(value = %steps_before, "steps before");

    info!("call defer_batch([(alice, {CLAIM_AMOUNT})]) [signer=oracle]");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), CLAIM_AMOUNT]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("view get_steps_since_tge [after]");
    let steps_after: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(
        steps_after.parse::<u64>()?,
        u64::from(CLAIM_AMOUNT),
        "steps_since_tge must advance by the batch step count on success"
    );
    info!(value = %steps_after, "steps after");

    info!("done");
    Ok(())
}

#[tokio::test]
#[tracing::instrument]
async fn test_defer_batch_rolls_back_steps_on_failed_record() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;

    info!("view get_steps_since_tge [before]");
    let steps_before: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(steps_before.parse::<u64>()?, 0);
    info!(value = %steps_before, "steps before");

    info!("call set_holding_account_id(alice) — a contractless account [signer=contract]");
    context
        .sweat
        .call("set_holding_account_id")
        .args_json(json!({ "account_id": context.alice.id() }))
        .transact()
        .await?
        .into_result()?;

    info!("call defer_batch([(alice, {CLAIM_AMOUNT})]) [signer=oracle] — record XCC will fail");
    let outcome = context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), CLAIM_AMOUNT]] }))
        .max_gas()
        .transact()
        .await?;

    outcome.clone().into_result()?;

    info!("assert the failure branch ran");
    assert!(
        outcome
            .logs()
            .iter()
            .any(|log| log.contains("rolled back steps counter")),
        "expected the on_record failure branch to log the rollback"
    );

    info!("view get_steps_since_tge [after failed defer]");
    let steps_after: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(
        steps_after.parse::<u64>()?,
        0,
        "steps_since_tge must be rolled back after a failed record XCC"
    );
    info!(value = %steps_after, "steps after");

    info!("view ft_balance_of(alice) + ft_balance_of(oracle)");
    let alice_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    let oracle_balance: String = context
        .sweat
        .view("ft_balance_of")
        .args_json(json!({ "account_id": context.oracle().id() }))
        .await?
        .json()?;
    assert_eq!(alice_balance, "0", "no tokens may be minted on a failed defer");
    assert_eq!(oracle_balance, "0", "no fee may be minted on a failed defer");

    info!("done");
    Ok(())
}
