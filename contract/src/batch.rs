//! Privileged batch operations over token accounts.
//!
//! Both methods are gated by the [`crate::Role::Oracle`] ACL role.
//!
//! - [`BatchApi::batch_ft_transfer`] moves tokens between arbitrary accounts
//!   without their signatures. Every transfer runs in the same function call,
//!   so the batch is atomic: one failing entry (insufficient balance,
//!   unregistered receiver, ...) panics and reverts the whole batch.
//!   Transfers are subject to the `token` pause but, unlike `ft_transfer`, not
//!   to the denylist.
//! - [`BatchApi::batch_storage_unregister`] removes zero-balance accounts from
//!   the token state. The holding account is always skipped. Unlike NEP-145 `storage_unregister`, it does **not**
//!   refund the storage deposit: the released NEAR stays on the contract
//!   account. No event is emitted: NEP-141 defines none for removing a
//!   zero-balance account.
//!
//! Every transfer emits its own standard NEP-141 `ft_transfer` event, so batch
//! size is bounded by the runtime log limits (100 logs and 16 KiB of log text
//! per receipt) as well as by gas.

use near_plugins::{access_control_any, AccessControllable};
use near_sdk::{json_types::U128, near, require, AccountId};

use crate::{api::BatchApi, Contract, ContractExt, Feature, Role};

#[near]
impl BatchApi for Contract {
    #[access_control_any(roles(Role::Oracle))]
    fn batch_ft_transfer(&mut self, transfers: Vec<(AccountId, AccountId, U128)>) {
        self.assert_feature_enabled(Feature::Token);
        require!(!transfers.is_empty(), "Empty transfers batch");

        for (sender_id, receiver_id, amount) in &transfers {
            self.token.internal_transfer(sender_id, receiver_id, amount.0, None);
        }
    }

    #[access_control_any(roles(Role::Oracle))]
    fn batch_storage_unregister(&mut self, account_ids: Vec<AccountId>) -> Vec<AccountId> {
        require!(!account_ids.is_empty(), "Empty accounts batch");

        let mut unregistered = Vec::new();
        for account_id in account_ids {
            // The holding account is never removed. `None` (not registered,
            // including an already removed duplicate) and positive balances
            // are skipped.
            if account_id != self.holding_account_id && self.token.accounts.get(&account_id) == Some(0) {
                self.token.accounts.remove(&account_id);
                unregistered.push(account_id);
            }
        }

        unregistered
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use near_contract_standards::{fungible_token::core::FungibleTokenCore, storage_management::StorageManagement};
    use near_plugins::AccessControllable;
    use near_sdk::{json_types::U128, test_utils::VMContextBuilder, testing_env, AccountId};

    use crate::{
        api::{BatchApi, PauseApi, RestrictionApi, SweatApi},
        Contract, Feature, Role,
    };

    fn account_id(account_id: &str) -> AccountId {
        AccountId::from_str(account_id).unwrap()
    }

    fn token() -> AccountId {
        account_id("sweat_the_token")
    }
    fn holding() -> AccountId {
        account_id("sweat_the_holding")
    }
    fn oracle() -> AccountId {
        account_id("sweat_the_oracle")
    }
    fn user1() -> AccountId {
        account_id("sweat_user1")
    }
    fn user2() -> AccountId {
        account_id("sweat_user2")
    }
    fn user3() -> AccountId {
        account_id("sweat_user3")
    }

    fn get_context(sender: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(token())
            .signer_account_id(sender.clone())
            .predecessor_account_id(sender);
        builder
    }

    /// super-admin is `token()`; `oracle()` holds `Oracle`; users 1-3
    /// are registered. Leaves the context with `oracle()` as the caller.
    fn setup() -> Contract {
        testing_env!(get_context(token()).build());
        let mut contract = Contract::new(holding(), token(), vec![], vec![], vec![], vec![]);
        contract.acl_grant_role(Role::Oracle.into(), oracle());
        contract.acl_grant_role(Role::DenylistManager.into(), token());
        contract.acl_grant_role(Role::PauseManager.into(), token());

        let min = contract.storage_balance_bounds().min;
        for user in [user1(), user2(), user3()] {
            testing_env!(get_context(token()).attached_deposit(min).build());
            contract.storage_deposit(Some(user), None);
        }

        testing_env!(get_context(oracle()).build());
        contract
    }

    fn deposit(contract: &mut Contract, account_id: &AccountId, amount: u128) {
        contract.token.internal_deposit(account_id, amount);
    }

    fn as_caller(sender: AccountId) {
        testing_env!(get_context(sender).build());
    }

    #[test]
    fn batch_ft_transfer_moves_balances() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        deposit(&mut contract, &user2(), 50);

        contract.batch_ft_transfer(vec![
            (user1(), user2(), U128(30)),
            (user2(), user3(), U128(70)),
            (user1(), user3(), U128(10)),
        ]);

        assert_eq!(contract.ft_balance_of(user1()).0, 60);
        assert_eq!(contract.ft_balance_of(user2()).0, 10);
        assert_eq!(contract.ft_balance_of(user3()).0, 80);
        assert_eq!(contract.ft_total_supply().0, 150);
    }

    #[test]
    fn batch_ft_transfer_emits_event_per_transfer() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        contract.batch_ft_transfer(vec![(user1(), user2(), U128(30)), (user1(), user3(), U128(20))]);

        let logs = near_sdk::test_utils::get_logs();
        assert_eq!(
            logs,
            vec![
                r#"EVENT_JSON:{"standard":"nep141","version":"1.0.0","event":"ft_transfer","data":[{"old_owner_id":"sweat_user1","new_owner_id":"sweat_user2","amount":"30"}]}"#,
                r#"EVENT_JSON:{"standard":"nep141","version":"1.0.0","event":"ft_transfer","data":[{"old_owner_id":"sweat_user1","new_owner_id":"sweat_user3","amount":"20"}]}"#,
            ]
        );
    }

    #[test]
    #[should_panic(expected = "Insufficient permissions for method batch_ft_transfer")]
    fn batch_ft_transfer_requires_role() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_caller(user1());
        contract.batch_ft_transfer(vec![(user1(), user2(), U128(30))]);
    }

    #[test]
    #[should_panic(expected = "Empty transfers batch")]
    fn batch_ft_transfer_rejects_empty_batch() {
        let mut contract = setup();
        contract.batch_ft_transfer(vec![]);
    }

    #[test]
    #[should_panic(expected = "The account doesn't have enough balance")]
    fn batch_ft_transfer_rejects_insufficient_balance() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        contract.batch_ft_transfer(vec![(user1(), user2(), U128(60)), (user1(), user3(), U128(60))]);
    }

    #[test]
    #[should_panic(expected = "The account sweat_unregistered is not registered")]
    fn batch_ft_transfer_rejects_unregistered_receiver() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        contract.batch_ft_transfer(vec![(user1(), account_id("sweat_unregistered"), U128(1))]);
    }

    #[test]
    #[should_panic(expected = "Sender and receiver should be different")]
    fn batch_ft_transfer_rejects_self_transfer() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        contract.batch_ft_transfer(vec![(user1(), user1(), U128(1))]);
    }

    #[test]
    #[should_panic(expected = "The amount should be a positive number")]
    fn batch_ft_transfer_rejects_zero_amount() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        contract.batch_ft_transfer(vec![(user1(), user2(), U128(0))]);
    }

    #[test]
    fn batch_ft_transfer_ignores_denylist() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        as_caller(token());
        contract.set_restricted(&user1(), true);
        contract.set_restricted(&user2(), true);

        as_caller(oracle());
        contract.batch_ft_transfer(vec![(user1(), user2(), U128(30))]);

        assert_eq!(contract.ft_balance_of(user1()).0, 70);
        assert_eq!(contract.ft_balance_of(user2()).0, 30);
    }

    #[test]
    #[should_panic(expected = "Feature 'token' is paused")]
    fn batch_ft_transfer_blocked_when_token_paused() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        as_caller(token());
        contract.pause_features(vec![Feature::Token]);

        as_caller(oracle());
        contract.batch_ft_transfer(vec![(user1(), user2(), U128(1))]);
    }

    #[test]
    fn batch_storage_unregister_removes_only_zero_balance_accounts() {
        let mut contract = setup();
        deposit(&mut contract, &user2(), 100);
        let unregistered_user = account_id("sweat_unregistered");

        let removed =
            contract.batch_storage_unregister(vec![user1(), user2(), unregistered_user.clone(), user3(), user1()]);

        assert_eq!(removed, vec![user1(), user3()]);
        assert!(contract.storage_balance_of(user1()).is_none());
        assert!(contract.storage_balance_of(user3()).is_none());
        assert!(contract.storage_balance_of(user2()).is_some());
        assert_eq!(contract.ft_balance_of(user2()).0, 100);
        assert!(contract.storage_balance_of(unregistered_user).is_none());
        assert!(near_sdk::test_utils::get_logs().is_empty());
    }

    #[test]
    fn batch_storage_unregister_skips_holding_account() {
        let mut contract = setup();
        let min = contract.storage_balance_bounds().min;
        testing_env!(get_context(token()).attached_deposit(min).build());
        contract.storage_deposit(Some(holding()), None);
        assert_eq!(contract.ft_balance_of(holding()).0, 0);

        as_caller(oracle());
        let removed = contract.batch_storage_unregister(vec![holding(), user1()]);

        assert_eq!(removed, vec![user1()]);
        assert!(contract.storage_balance_of(holding()).is_some());
    }

    #[test]
    fn batch_storage_unregister_without_matches_removes_nothing() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        let removed = contract.batch_storage_unregister(vec![user1(), account_id("sweat_unregistered")]);

        assert!(removed.is_empty());
        assert_eq!(contract.ft_balance_of(user1()).0, 100);
    }

    #[test]
    #[should_panic(expected = "Insufficient permissions for method batch_storage_unregister")]
    fn batch_storage_unregister_requires_role() {
        let mut contract = setup();

        as_caller(user1());
        contract.batch_storage_unregister(vec![user1()]);
    }

    #[test]
    #[should_panic(expected = "Empty accounts batch")]
    fn batch_storage_unregister_rejects_empty_batch() {
        let mut contract = setup();
        contract.batch_storage_unregister(vec![]);
    }

    #[test]
    fn unregistered_account_can_register_again() {
        let mut contract = setup();
        contract.batch_storage_unregister(vec![user1()]);

        let min = contract.storage_balance_bounds().min;
        testing_env!(get_context(user1()).attached_deposit(min).build());
        contract.storage_deposit(None, None);

        assert_eq!(contract.storage_balance_of(user1()).map(|b| b.total), Some(min));
        assert_eq!(contract.ft_balance_of(user1()).0, 0);
    }
}
