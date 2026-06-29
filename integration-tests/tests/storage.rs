use near_workspaces::types::NearToken;
use serde_json::{json, Value};
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::Context};

/// Unregistering is intentionally disabled: the Sweat Foundation subsidizes the
/// storage of accounts created with Sweat Wallet, so `storage_unregister` must
/// never let a user reclaim their staked funds. It panics for every `force`
/// value and leaves the account registered.
#[tokio::test]
#[tracing::instrument]
async fn test_storage_unregister_is_disabled() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("view storage_balance_of(alice) — registered during setup");
    let balance_before: Option<Value> = context
        .sweat
        .view("storage_balance_of")
        .args_json(json!({ "account_id": context.alice.id() }))
        .await?
        .json()?;
    assert!(balance_before.is_some(), "alice must be registered before the call");
    info!("storage_balance_of: {balance_before:?}");

    for force in [None, Some(false), Some(true)] {
        info!("call storage_unregister(force={force:?}) [signer=alice]");
        let result = context
            .alice
            .call(context.sweat.id(), "storage_unregister")
            .args_json(json!({ "force": force }))
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await?
            .into_result();
        assert!(
            result.has_panic("storage_unregister is disabled"),
            "storage_unregister must panic (force={force:?})"
        );
        info!("storage_unregister: {result:?}");

        info!("view storage_balance_of(alice) — must stay registered");
        let balance_after: Option<Value> = context
            .sweat
            .view("storage_balance_of")
            .args_json(json!({ "account_id": context.alice.id() }))
            .await?
            .json()?;
        assert_eq!(
            balance_after, balance_before,
            "alice's storage balance must be untouched (force={force:?})"
        );
        info!("storage_balance_of: {balance_after:?}");
    }

    Ok(())
}
