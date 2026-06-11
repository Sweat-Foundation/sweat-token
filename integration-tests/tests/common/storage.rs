use anyhow::{anyhow, Result};
use near_workspaces::{types::NearToken, AccountId, Contract};
use serde_json::{json, Value};

pub async fn storage_balance_min(ft: &Contract) -> Result<NearToken> {
    let bounds: Value = ft.view("storage_balance_bounds").await?.json()?;
    let min_str = bounds
        .get("min")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("storage_balance_bounds.min missing"))?;
    Ok(NearToken::from_yoctonear(min_str.parse()?))
}

pub async fn storage_deposit(ft: &Contract, account_id: &AccountId, deposit: NearToken) -> Result<()> {
    ft.call("storage_deposit")
        .args_json(json!({ "account_id": account_id }))
        .deposit(deposit)
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

pub async fn register_for_storage(ft: &Contract, account_id: &AccountId) -> Result<()> {
    let min = storage_balance_min(ft).await?;
    storage_deposit(ft, account_id, min).await
}
