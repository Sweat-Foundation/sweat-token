use std::path::PathBuf;

use anyhow::{anyhow, Result};
use near_workspaces::{network::Sandbox, types::NearToken, Account, AccountId, Contract, Worker};
use serde_json::json;
use tracing::info;

use super::helpers::init_tracing;
use super::storage::{storage_balance_min, storage_deposit};

const FT_POSTFIX: &str = ".u.sweat.testnet";
const INITIAL_USER_BALANCE: NearToken = NearToken::from_near(10);

const SWEAT_WASM_ENV: &str = "SWEAT_WASM";
const CLAIM_WASM_ENV: &str = "CLAIM_WASM";
const STUB_WASM_ENV: &str = "STUB_WASM";

pub struct Context {
    #[allow(dead_code)] // held to keep the sandbox alive for the test's lifetime
    pub worker: Worker<Sandbox>,
    pub sweat: Contract,
    pub alice: Account,
    oracle: Option<Account>,
    bob: Option<Account>,
    claim: Option<Contract>,
    stub: Option<Contract>,
}

impl Context {
    pub fn builder() -> ContextBuilder {
        ContextBuilder::new()
    }

    pub fn oracle(&self) -> &Account {
        self.oracle
            .as_ref()
            .expect("oracle missing — add .with_oracle() to the Context::builder() chain")
    }

    pub fn bob(&self) -> &Account {
        self.bob
            .as_ref()
            .expect("bob missing — add .with_bob() to the Context::builder() chain")
    }

    pub fn claim(&self) -> &Contract {
        self.claim
            .as_ref()
            .expect("claim missing — add .with_claim() to the Context::builder() chain")
    }

    pub fn stub(&self) -> &Contract {
        self.stub
            .as_ref()
            .expect("stub missing — add .with_stub() to the Context::builder() chain")
    }
}

pub struct ContextBuilder {
    oracle: bool,
    bob: bool,
    claim: bool,
    stub: bool,
}

impl ContextBuilder {
    fn new() -> Self {
        init_tracing();
        Self {
            oracle: false,
            bob: false,
            claim: false,
            stub: false,
        }
    }

    pub fn with_oracle(mut self) -> Self {
        self.oracle = true;
        self
    }

    pub fn with_bob(mut self) -> Self {
        self.bob = true;
        self
    }

    pub fn with_claim(mut self) -> Self {
        self.claim = true;
        self
    }

    pub fn with_stub(mut self) -> Self {
        self.stub = true;
        self
    }

    #[tracing::instrument(name = "prepare", skip(self))]
    pub async fn build(self) -> Result<Context> {
        info!("booting sandbox");
        let worker = near_workspaces::sandbox().await?;
        let root = worker.root_account()?;

        info!("deploying sweat");
        let sweat = deploy(&worker, sweat_wasm_path(), "sweat", SWEAT_WASM_ENV).await?;
        info!(id = %sweat.id(), "sweat deployed");

        // Deploy the claim contract before initializing sweat so its account id
        // can be baked in as the fixed deferred-minting holding account.
        let claim = if self.claim {
            info!("deploying claim");
            let claim = deploy(&worker, claim_wasm_path(), "claim", CLAIM_WASM_ENV).await?;
            info!(id = %claim.id(), "claim deployed");
            Some(claim)
        } else {
            None
        };

        info!("initializing sweat (new)");
        sweat
            .call("new")
            .args_json(json!({
                "postfix": FT_POSTFIX,
                "holding_account_id": claim.as_ref().map(Contract::id),
            }))
            .transact()
            .await?
            .into_result()?;

        info!("creating alice");
        let alice = create_user(&root, "alice").await?;

        let min = storage_balance_min(&sweat).await?;
        storage_deposit(&sweat, alice.id(), min).await?;

        let oracle = if self.oracle {
            info!("creating oracle + sweat.acl_grant_role(Oracle)");
            let oracle = create_user(&root, "oracle").await?;
            sweat
                .call("acl_grant_role")
                .args_json(json!({
                    "role": "Oracle",
                    "account_id": oracle.id(),
                }))
                .transact()
                .await?
                .into_result()?;
            storage_deposit(&sweat, oracle.id(), min).await?;
            Some(oracle)
        } else {
            None
        };

        let bob = if self.bob {
            info!("creating bob (FT-unregistered)");
            Some(create_user(&root, "bob").await?)
        } else {
            None
        };

        // Finish claim setup now that sweat is initialized.
        if let Some(claim) = &claim {
            info!("initializing claim");
            init_claim(claim, sweat.id()).await?;
            storage_deposit(&sweat, claim.id(), min).await?;
        }

        let stub = if self.stub {
            info!("deploying + initializing stub");
            let stub = deploy(&worker, stub_wasm_path(), "stub", STUB_WASM_ENV).await?;
            info!(id = %stub.id(), "stub deployed");
            stub.call("new").transact().await?.into_result()?;
            Some(stub)
        } else {
            None
        };

        info!("ready");
        Ok(Context {
            worker,
            sweat,
            alice,
            oracle,
            bob,
            claim,
            stub,
        })
    }
}

fn wasm_path(env_var: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(env_var).map(PathBuf::from).unwrap_or(default)
}

fn sweat_wasm_path() -> PathBuf {
    wasm_path(
        SWEAT_WASM_ENV,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("integration-wasm")
            .join("contract.wasm"),
    )
}

fn claim_wasm_path() -> PathBuf {
    wasm_path(
        CLAIM_WASM_ENV,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("res")
            .join("sweat_claim.wasm"),
    )
}

fn stub_wasm_path() -> PathBuf {
    wasm_path(
        STUB_WASM_ENV,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("res")
            .join("exploit_stub.wasm"),
    )
}

async fn deploy(worker: &Worker<Sandbox>, path: PathBuf, label: &str, env_var: &str) -> Result<Contract> {
    let bytes = std::fs::read(&path).map_err(|e| {
        anyhow!(
            "failed to read {label} WASM at {} — did you run `make build-integration`? \
             Override the path with the {env_var} env var. ({e})",
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
