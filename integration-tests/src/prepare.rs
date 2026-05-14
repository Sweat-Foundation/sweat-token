use std::path::PathBuf;

use anyhow::{anyhow, Result};
use near_workspaces::{network::Sandbox, types::NearToken, Account, AccountId, Contract, Worker};
use serde_json::{json, Value};

use crate::helpers::step;

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
    let sweat = deploy(&worker, SWEAT_WASM).await?;
    step!(&tag, "  sweat → {}", sweat.id());
    let claim = deploy(&worker, CLAIM_WASM).await?;
    step!(&tag, "  claim → {}", claim.id());
    let stub = deploy(&worker, STUB_WASM).await?;
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

async fn deploy(worker: &Worker<Sandbox>, wasm_name: &str) -> Result<Contract> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("res")
        .join(wasm_name);
    let bytes = std::fs::read(&path)
        .map_err(|e| anyhow!("failed to read {}: {e}", path.display()))?;
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

async fn storage_balance_min(ft: &Contract) -> Result<NearToken> {
    let bounds: Value = ft.view("storage_balance_bounds").await?.json()?;
    let min_str = bounds
        .get("min")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("storage_balance_bounds.min missing"))?;
    Ok(NearToken::from_yoctonear(min_str.parse()?))
}

async fn storage_deposit(ft: &Contract, account_id: &AccountId, deposit: NearToken) -> Result<()> {
    ft.call("storage_deposit")
        .args_json(json!({ "account_id": account_id }))
        .deposit(deposit)
        .transact()
        .await?
        .into_result()?;
    Ok(())
}
