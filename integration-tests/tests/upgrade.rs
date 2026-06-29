use serde_json::json;
use tracing::info;

mod common;
use common::{panic::PanicFinder, prepare::sweat_wasm_bytes, prepare::Context};

/// `up_stage_code` is restricted to `StagingManager` (the `code_stagers` role)
/// and `up_deploy_code` to `UpgradeManager` (`code_deployers`). The two roles
/// are distinct: holding the stager role does not grant the deployer one.
#[tokio::test]
#[tracing::instrument]
async fn test_upgrade_access_control() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;
    let code = sweat_wasm_bytes()?;

    info!("call up_stage_code [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "up_stage_code")
        .args(code.clone())
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method up_stage_code restricted by access control."));

    info!("view up_staged_code_hash — nothing should be staged yet");
    let staged: Option<String> = context.sweat.view("up_staged_code_hash").await?.json()?;
    assert_eq!(staged, None, "unauthorized staging must not store any code");

    info!("call up_deploy_code [signer=alice, unauthorized]");
    let result = context
        .alice
        .call(context.sweat.id(), "up_deploy_code")
        .args_json(json!({ "hash": "ignored", "function_call_args": null }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(result.has_panic("Insufficient permissions for method up_deploy_code restricted by access control."));

    info!("call acl_grant_role(StagingManager, alice) [signer=contract, super-admin]");
    let granted: Option<bool> = context
        .sweat
        .call("acl_grant_role")
        .args_json(json!({ "role": "StagingManager", "account_id": context.alice.id() }))
        .transact()
        .await?
        .json()?;
    assert_eq!(granted, Some(true));

    info!("call up_stage_code [signer=alice, authorized as StagingManager]");
    let result = context
        .alice
        .call(context.sweat.id(), "up_stage_code")
        .args(code.clone())
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success());

    info!("view up_staged_code_hash — code is now staged");
    let staged: Option<String> = context.sweat.view("up_staged_code_hash").await?.json()?;
    assert!(staged.is_some(), "staged code hash should be set after staging");

    info!("call up_deploy_code [signer=alice, StagingManager but not UpgradeManager]");
    let result = context
        .alice
        .call(context.sweat.id(), "up_deploy_code")
        .args_json(json!({ "hash": staged.unwrap(), "function_call_args": null }))
        .max_gas()
        .transact()
        .await?
        .into_result();
    assert!(
        result.has_panic("Insufficient permissions for method up_deploy_code restricted by access control."),
        "the stager role must not grant deploy permission"
    );

    Ok(())
}

/// Full stage → deploy flow: an `UpgradeManager` re-deploys the contract over
/// itself and the existing state survives the upgrade.
#[tokio::test]
#[tracing::instrument]
async fn test_upgrade_deploy() -> anyhow::Result<()> {
    let context = Context::builder().with_oracle().with_claim().build().await?;
    let code = sweat_wasm_bytes()?;

    info!("record a batch so there is pre-upgrade state to preserve");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    let steps_before: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_ne!(steps_before, "0", "steps should be recorded before the upgrade");

    info!("grant the upgrade roles to alice [signer=contract, super-admin]");
    for role in ["StagingManager", "UpgradeManager"] {
        let granted: Option<bool> = context
            .sweat
            .call("acl_grant_role")
            .args_json(json!({ "role": role, "account_id": context.alice.id() }))
            .transact()
            .await?
            .json()?;
        assert_eq!(granted, Some(true), "{role} grant should succeed");
    }

    info!("stage the contract code [signer=alice, StagingManager]");
    context
        .alice
        .call(context.sweat.id(), "up_stage_code")
        .args(code)
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("read back the staged code hash to feed the deploy");
    let staged_hash: Option<String> = context.sweat.view("up_staged_code_hash").await?.json()?;
    let staged_hash = staged_hash.expect("code must be staged before deploy");

    info!("deploy the staged code [signer=alice, UpgradeManager]");
    let result = context
        .alice
        .call(context.sweat.id(), "up_deploy_code")
        .args_json(json!({ "hash": staged_hash, "function_call_args": null }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    assert!(result.outcome().is_success(), "deploy should succeed");

    info!("verify state survived the upgrade");
    let steps_after: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(steps_after, steps_before, "steps counter must be preserved across the upgrade");

    info!("verify the upgraded contract still serves authorized calls");
    context
        .oracle()
        .call(context.sweat.id(), "defer_batch")
        .args_json(json!({ "steps_batch": [[context.alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    let steps_final: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    assert_eq!(steps_final, "20000", "the upgraded contract should keep recording steps");

    Ok(())
}
