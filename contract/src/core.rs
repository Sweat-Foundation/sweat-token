use near_contract_standards::fungible_token::{core::FungibleTokenCore, resolver::FungibleTokenResolver, Balance};
use near_sdk::{env, json_types::U128, near, AccountId, PromiseOrValue};

use crate::{Contract, ContractExt, Feature};

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.assert_feature_enabled(Feature::Token);
        self.assert_not_in_denylist(vec![&env::predecessor_account_id(), &receiver_id]);

        self.token.ft_transfer(receiver_id, amount, memo);
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.assert_feature_enabled(Feature::Token);
        self.assert_not_in_denylist(vec![&env::predecessor_account_id(), &receiver_id]);

        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for Contract {
    #[private]
    fn ft_resolve_transfer(&mut self, sender_id: AccountId, receiver_id: AccountId, amount: U128) -> U128 {
        let (used_amount, _) = self.token.internal_ft_resolve_transfer(&sender_id, receiver_id, amount);
        used_amount.into()
    }
}

/// Deposit primitive that does **not** require the recipient to have an existing
/// storage-registered account: if no balance entry exists, it starts from zero
/// instead of panicking.
///
/// Taken from contract standards but modified to default if an account isn't
/// initialized rather than panicking:
/// <https://github.com/near/near-sdk-rs/blob/6596dc311036fe51d94358ac8f6497ef6e5a7cfc/near-contract-standards/src/fungible_token/core_impl.rs#L105>
///
/// # Safety contract
///
/// Bypassing the storage-registration check is only sound when the recipient is
/// a **trusted protocol account configured by the contract owner**. Callers
/// must restrict use to:
///
/// - the **fee account** — an [`crate::Role::Oracle`] grantee that submitted the
///   batch via `defer_batch` and earns the minting fee, and
/// - the **holding account** — set via the private
///   [`crate::api::SweatApi::set_holding_account_id`] and used to hold newly
///   minted user rewards.
///
/// Both are controlled by the protocol, so allowing them to skip registration
/// cannot be used by untrusted parties to occupy contract storage without
/// paying for it. Do **not** call this for arbitrary user-supplied accounts;
/// use the standard `FungibleToken` transfer paths, which enforce registration.
pub(crate) trait InternalDeposit {
    fn internal_deposit(&mut self, account_id: &AccountId, amount: Balance);
}

impl InternalDeposit for Contract {
    fn internal_deposit(&mut self, account_id: &AccountId, amount: Balance) {
        let balance = self.token.accounts.get(account_id).unwrap_or_default();
        let new_balance = balance
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Balance overflow"));
        self.token.accounts.insert(account_id, &new_balance);
        self.token.total_supply = self
            .token
            .total_supply
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("Total supply overflow"));
    }
}
