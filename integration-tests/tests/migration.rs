use std::path::PathBuf;

use near_workspaces::{types::NearToken, Account};
use serde_json::json;
use tracing::info;

mod common;
use common::helpers::init_tracing;

const FT_POSTFIX: &str = ".u.sweat.testnet";
const INITIAL_USER_BALANCE: NearToken = NearToken::from_near(10);

fn old_wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("res")
        .join("sweat_old.wasm")
}

fn new_wasm_path() -> PathBuf {
    std::env::var_os("SWEAT_WASM").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("integration-wasm")
            .join("contract.wasm")
    })
}

async fn create_user(root: &Account, name: &str) -> anyhow::Result<Account> {
    Ok(root
        .create_subaccount(name)
        .initial_balance(INITIAL_USER_BALANCE)
        .transact()
        .await?
        .into_result()?)
}

#[tokio::test]
#[tracing::instrument]
async fn migration_from_deployed_release() -> anyhow::Result<()> {
    init_tracing();

    info!("booting sandbox");
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    info!("deploying old contract");
    let old_wasm = std::fs::read(old_wasm_path())?;
    let contract = worker.dev_deploy(&old_wasm).await?;

    contract
        .call("new")
        .args_json(json!({ "postfix": FT_POSTFIX }))
        .transact()
        .await?
        .into_result()?;

    let oracle1 = create_user(&root, "oracle1").await?;
    let oracle2 = create_user(&root, "oracle2").await?;
    let alice = create_user(&root, "alice").await?;
    let denied = create_user(&root, "denied").await?;
    let admin = create_user(&root, "admin").await?;
    let denylist_manager = create_user(&root, "denylist-manager").await?;
    let pause_manager = create_user(&root, "pause-manager").await?;
    let unpause_manager = create_user(&root, "unpause-manager").await?;

    info!("registering oracles on the old contract");
    for oracle in [&oracle1, &oracle2] {
        contract
            .call("add_oracle")
            .args_json(json!({ "account_id": oracle.id() }))
            .transact()
            .await?
            .into_result()?;
    }

    info!("denylisting an account on the old contract");
    contract
        .call("set_restricted")
        .args_json(json!({ "account_id": denied.id(), "is_restricted": true }))
        .transact()
        .await?
        .into_result()?;

    info!("recording a batch on the old contract");
    oracle1
        .call(contract.id(), "record_batch")
        .args_json(json!({ "steps_batch": [[alice.id(), 10_000]] }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let oracles_before: Vec<String> = contract.view("get_oracles").await?.json()?;
    assert_eq!(oracles_before.len(), 2, "old contract should have 2 oracles");
    let denied_restricted_before: bool = contract
        .view("is_restricted")
        .args_json(json!({ "account_id": denied.id() }))
        .await?
        .json()?;
    assert!(denied_restricted_before, "denied account should be restricted before migration");
    let steps_before: String = contract.view("get_steps_since_tge").await?.json()?;
    let alice_balance_before: String = contract
        .view("ft_balance_of")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    assert_ne!(
        alice_balance_before, "0",
        "alice should have a balance before migration"
    );
    info!(steps = %steps_before, balance = %alice_balance_before, "pre-migration snapshot");

    info!("deploying new contract over the same account");
    let new_wasm = std::fs::read(new_wasm_path())?;
    contract.as_account().deploy(&new_wasm).await?.into_result()?;

    info!("calling migrate [signer=contract account]");
    contract
        .call("migrate")
        .args_json(json!({
            "holding_account_id": root.id(),
            "super_admin_account_id": admin.id(),
            "denylist_manager_account_ids": [denylist_manager.id()],
            "pause_manager_account_ids": [pause_manager.id()],
            "unpause_manager_account_ids": [unpause_manager.id()],
        }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    info!("verifying super admin");
    let is_super_admin: bool = contract
        .view("acl_is_super_admin")
        .args_json(json!({ "account_id": admin.id() }))
        .await?
        .json()?;
    assert!(is_super_admin, "admin account should be the ACL super admin");

    info!("verifying role managers migrated to their ACL roles");
    for (role, manager) in [
        ("DenylistManager", &denylist_manager),
        ("PauseManager", &pause_manager),
        ("UnpauseManager", &unpause_manager),
    ] {
        let has_role: bool = contract
            .view("acl_has_role")
            .args_json(json!({ "role": role, "account_id": manager.id() }))
            .await?
            .json()?;
        assert!(has_role, "{} should hold the {} role", manager.id(), role);
    }

    info!("verifying oracles migrated to the ACL Oracle role");
    let grantees: Vec<String> = contract
        .view("acl_get_grantees")
        .args_json(json!({ "role": "Oracle", "skip": 0, "limit": 100 }))
        .await?
        .json()?;
    assert_eq!(grantees.len(), 2, "both oracles should hold the Oracle role");
    for oracle in [&oracle1, &oracle2] {
        let has_role: bool = contract
            .view("acl_has_role")
            .args_json(json!({ "role": "Oracle", "account_id": oracle.id() }))
            .await?
            .json()?;
        assert!(has_role, "{} should hold the Oracle role", oracle.id());
    }

    info!("verifying the denylist survived the migration");
    let denied_restricted_after: bool = contract
        .view("is_restricted")
        .args_json(json!({ "account_id": denied.id() }))
        .await?
        .json()?;
    assert!(
        denied_restricted_after,
        "denylisted account must stay restricted after migration"
    );

    info!("verifying token state survived");
    let steps_after: String = contract.view("get_steps_since_tge").await?.json()?;
    assert_eq!(steps_after, steps_before, "steps counter must be preserved");
    let alice_balance_after: String = contract
        .view("ft_balance_of")
        .args_json(json!({ "account_id": alice.id() }))
        .await?
        .json()?;
    assert_eq!(alice_balance_after, alice_balance_before, "balance must be preserved");

    info!("verifying the legacy get_oracles view is gone");
    assert!(
        contract.view("get_oracles").await.is_err(),
        "get_oracles should be removed from the new contract"
    );

    info!("done");
    Ok(())
}
