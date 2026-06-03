use near_sdk::{
    json_types::{U128, U64},
    AccountId, PromiseOrValue,
};

pub trait SweatApi {
    fn new(
        postfix: Option<String>,
        holding_account_id: AccountId,
        super_admin_account_id: AccountId,
        oracle_account_ids: Vec<AccountId>,
        denylist_manager_account_ids: Vec<AccountId>,
        pause_manager_account_ids: Vec<AccountId>,
        unpause_manager_account_ids: Vec<AccountId>,
    ) -> Self;
    /// Sets the trusted holding-account contract that will custody the user
    /// portion of every minted batch until end users claim it. See the
    /// [`crate::defer`] module docs for the full flow.
    fn set_holding_account_id(&mut self, account_id: AccountId);
    fn get_holding_account_id(&self) -> AccountId;
    fn burn(&mut self, amount: U128);
    fn get_steps_since_tge(&self) -> U64;
    fn formula(&self, steps_since_tge: U64, steps: u32) -> U128;
}

pub trait RestrictionApi {
    fn is_restricted(&self, account_id: &AccountId) -> bool;
    fn set_restricted(&mut self, account_id: &AccountId, is_restricted: bool);
}

/// Token-minting API.
///
/// Implementations mint user rewards to the configured holding account rather
/// than directly to end users. See the [`crate::defer`] module docs for the
/// full claim flow and trust model.
pub trait SweatDefer {
    fn defer_batch(&mut self, steps_batch: Vec<(AccountId, u32)>) -> PromiseOrValue<()>;
}

pub struct Payout {
    pub amount_for_user: u128,
    pub fee: u128,
}

impl From<u128> for Payout {
    fn from(value: u128) -> Self {
        let fee = value.div_ceil(20); // == value * 5 / 100

        Self {
            fee,
            amount_for_user: value - fee,
        }
    }
}
