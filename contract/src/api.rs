use near_sdk::{
    json_types::{U128, U64},
    AccountId, PromiseOrValue,
};

use crate::pause::Feature;

pub trait SweatApi {
    fn new(
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

/// Per-feature pause controls. See the [`crate::pause`] module.
pub trait PauseApi {
    /// Pauses each of `features`. Requires the `PauseManager` role. Returns
    /// whether the paused set changed.
    fn pause_features(&mut self, features: Vec<Feature>) -> bool;
    /// Unpauses each of `features`. Requires the `UnpauseManager` role. Returns
    /// whether the paused set changed.
    fn unpause_features(&mut self, features: Vec<Feature>) -> bool;
    fn is_feature_paused(&self, feature: Feature) -> bool;
    fn get_paused_features(&self) -> Vec<Feature>;
}

/// Token-minting API.
///
/// Implementations mint user rewards to the configured holding account rather
/// than directly to end users. See the [`crate::defer`] module docs for the
/// full claim flow and trust model.
pub trait SweatDefer {
    fn defer_batch(&mut self, steps_batch: Vec<(AccountId, u32)>) -> PromiseOrValue<()>;
}

/// Privileged batch operations over token accounts. Every method requires the
/// [`crate::Role::Oracle`] role. See the [`crate::batch`] module.
pub trait BatchApi {
    /// Moves tokens between arbitrary accounts. Each entry is
    /// `(sender_id, receiver_id, amount)`. The whole batch is atomic: if any
    /// transfer fails, the call panics and no transfer is applied.
    fn batch_ft_transfer(&mut self, transfers: Vec<(AccountId, AccountId, U128)>);
    /// Removes the storage registration of every account in `account_ids` whose
    /// SWEAT balance is zero. The holding account, accounts with a positive
    /// balance and unregistered accounts are skipped. Returns the accounts that were actually removed.
    fn batch_storage_unregister(&mut self, account_ids: Vec<AccountId>) -> Vec<AccountId>;
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
