use std::path::PathBuf;

use anyhow::{anyhow, Result};
use near_workspaces::{network::Sandbox, types::NearToken, Account, AccountId, Contract, Worker};
use serde_json::json;

use crate::helpers::step;
use crate::storage::{storage_balance_min, storage_deposit};

const FT_POSTFIX: &str = ".u.sweat.testnet";
const LONG_ACCOUNT_NAME: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INITIAL_USER_BALANCE: NearToken = NearToken::from_near(10);

const SWEAT_WASM: &str = "sweat.wasm";
const CLAIM_WASM: &str = "sweat_claim.wasm";
const STUB_WASM: &str = "exploit_stub.wasm";

pub struct Context {
    pub worker: Worker<Sandbox>,
    pub sweat: Contract,
    pub claim: Contract,
    pub stub: Contract,
    pub oracle: Account,
    pub alice: Account,
    pub bob: Account,
    pub long: Account,
}

pub async fn prepare_contract(caller: &str) -> Result<Context> {
    let tag = format!("{caller}/prepare");

    step!(&tag, "booting sandbox");
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    step!(&tag, "deploying contracts");
    let sweat = deploy(&worker, integration_wasm_path(), "sweat").await?;
    step!(&tag, "  sweat → {}", sweat.id());
    let claim = deploy(&worker, res_wasm_path(CLAIM_WASM), "claim").await?;
    step!(&tag, "  claim → {}", claim.id());
    let stub = deploy(&worker, res_wasm_path(STUB_WASM), "stub").await?;
    step!(&tag, "  stub  → {}", stub.id());

    step!(&tag, "creating user accounts");
    let oracle = create_user(&root, "oracle").await?;
    let alice = create_user(&root, "alice").await?;
    let bob = create_user(&root, "bob").await?;
    let long = create_user(&root, LONG_ACCOUNT_NAME).await?;

    step!(&tag, "initializing sweat (new + add_oracle)");
    init_sweat(&sweat, oracle.id()).await?;
    step!(&tag, "initializing stub (new)");
    init_stub(&stub).await?;
    step!(&tag, "initializing claim (init + add_oracle)");
    init_claim(&claim, sweat.id()).await?;

    step!(&tag, "registering accounts for FT storage");
    let min = storage_balance_min(&sweat).await?;
    storage_deposit(&sweat, oracle.id(), min).await?;
    storage_deposit(&sweat, alice.id(), min).await?;
    storage_deposit(&sweat, long.id(), min).await?;
    storage_deposit(&sweat, claim.id(), min).await?;

    step!(&tag, "ready");
    Ok(Context {
        worker,
        sweat,
        claim,
        stub,
        oracle,
        alice,
        bob,
        long,
    })
}

fn integration_wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("integration-wasm")
        .join(SWEAT_WASM)
}

fn res_wasm_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("res")
        .join(name)
}

async fn deploy(worker: &Worker<Sandbox>, path: PathBuf, label: &str) -> Result<Contract> {
    let bytes = std::fs::read(&path).map_err(|e| {
        anyhow!(
            "failed to read {label} WASM at {} — did you run `make build-integration`? ({e})",
            path.display()
        )
    })?;
    Ok(worker.dev_deploy(&bytes).await?)
}

async fn create_user(root: &Account, name: &str) -> Result<Account> {
    Ok(root
        .create_subaccount(name)
        .initial_balance(INITIAL_USER_BALANCE)
        .transact()
        .await?
        .into_result()?)
}

async fn init_sweat(sweat: &Contract, oracle_id: &AccountId) -> Result<()> {
    sweat
        .call("new")
        .args_json(json!({ "postfix": FT_POSTFIX }))
        .transact()
        .await?
        .into_result()?;

    sweat
        .call("add_oracle")
        .args_json(json!({ "account_id": oracle_id }))
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

async fn init_stub(stub: &Contract) -> Result<()> {
    stub.call("new").transact().await?.into_result()?;
    Ok(())
}

async fn init_claim(claim: &Contract, token_account_id: &AccountId) -> Result<()> {
    claim
        .call("init")
        .args_json(json!({ "token_account_id": token_account_id }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    claim
        .call("add_oracle")
        .args_json(json!({ "account_id": token_account_id }))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}
