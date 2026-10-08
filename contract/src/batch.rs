//! Batch operations over token accounts.
//!
//! - [`BatchApi::batch_ft_transfer`] is a multi-receiver `ft_transfer`: the
//!   caller sends tokens from its own account to several receivers in one
//!   call. It has the same requirements as `ft_transfer` (1 yoctoNEAR deposit,
//!   `token` pause, denylist on the sender and every receiver, registered
//!   receivers). The batch is atomic: one failing entry panics and reverts the
//!   whole batch. All transfers are reported in a single NEP-141
//!   `ft_transfer` event whose `data` array holds one entry per transfer, so
//!   batch size is bounded by gas and the 16 KiB per-receipt log limit.
//! - [`BatchApi::batch_storage_unregister`] is gated by the
//!   [`crate::Role::Oracle`] ACL role and removes zero-balance accounts from
//!   the token state. The holding account is always skipped. Unlike NEP-145
//!   `storage_unregister`, it does **not** refund the storage deposit: the
//!   released NEAR stays on the contract account. No event is emitted: NEP-141
//!   defines none for removing a zero-balance account.

use near_contract_standards::fungible_token::events::FtTransfer;
use near_plugins::{access_control_any, AccessControllable};
use near_sdk::{assert_one_yocto, env, json_types::U128, near, require, AccountId};

use crate::{api::BatchApi, Contract, ContractExt, Feature, Role};

#[near]
impl BatchApi for Contract {
    #[payable]
    fn batch_ft_transfer(&mut self, transfers: Vec<(AccountId, U128)>, memo: Option<String>) {
        self.assert_feature_enabled(Feature::Token);
        assert_one_yocto();
        require!(!transfers.is_empty(), "Empty transfers batch");

        let sender_id = env::predecessor_account_id();
        self.assert_not_in_denylist(vec![&sender_id]);

        for (receiver_id, amount) in &transfers {
            self.assert_not_in_denylist(vec![receiver_id]);
            require!(&sender_id != receiver_id, "Sender and receiver should be different");
            require!(amount.0 > 0, "The amount should be a positive number");

            self.token.internal_withdraw(&sender_id, amount.0);
            self.token.internal_deposit(receiver_id, amount.0);
        }

        let events: Vec<FtTransfer> = transfers
            .iter()
            .map(|(receiver_id, amount)| FtTransfer {
                old_owner_id: &sender_id,
                new_owner_id: receiver_id,
                amount: *amount,
                memo: memo.as_deref(),
            })
            .collect();
        FtTransfer::emit_many(&events);
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
    use near_sdk::{json_types::U128, test_utils::VMContextBuilder, testing_env, AccountId, NearToken};

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

    /// Calls as `sender` with the 1 yoctoNEAR deposit `batch_ft_transfer` requires.
    fn as_sender(sender: AccountId) {
        testing_env!(get_context(sender).attached_deposit(NearToken::from_yoctonear(1)).build());
    }

    #[test]
    fn batch_ft_transfer_moves_balances() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        deposit(&mut contract, &user2(), 50);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(30)), (user3(), U128(10)), (user2(), U128(5))], None);

        assert_eq!(contract.ft_balance_of(user1()).0, 55);
        assert_eq!(contract.ft_balance_of(user2()).0, 85);
        assert_eq!(contract.ft_balance_of(user3()).0, 10);
        assert_eq!(contract.ft_total_supply().0, 150);
    }

    #[test]
    fn batch_ft_transfer_emits_single_event_with_all_transfers() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(30)), (user3(), U128(20))], Some("hi".to_string()));

        assert_eq!(
            near_sdk::test_utils::get_logs(),
            vec![
                r#"EVENT_JSON:{"standard":"nep141","version":"1.0.0","event":"ft_transfer","data":[{"old_owner_id":"sweat_user1","new_owner_id":"sweat_user2","amount":"30","memo":"hi"},{"old_owner_id":"sweat_user1","new_owner_id":"sweat_user3","amount":"20","memo":"hi"}]}"#,
            ]
        );
    }

    #[test]
    #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
    fn batch_ft_transfer_requires_one_yocto() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_caller(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(30))], None);
    }

    #[test]
    #[should_panic(expected = "Empty transfers batch")]
    fn batch_ft_transfer_rejects_empty_batch() {
        let mut contract = setup();
        as_sender(user1());
        contract.batch_ft_transfer(vec![], None);
    }

    #[test]
    #[should_panic(expected = "The account doesn't have enough balance")]
    fn batch_ft_transfer_rejects_insufficient_balance() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(60)), (user3(), U128(60))], None);
    }

    #[test]
    #[should_panic(expected = "The account sweat_unregistered is not registered")]
    fn batch_ft_transfer_rejects_unregistered_receiver() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(account_id("sweat_unregistered"), U128(1))], None);
    }

    #[test]
    #[should_panic(expected = "Sender and receiver should be different")]
    fn batch_ft_transfer_rejects_self_transfer() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user1(), U128(1))], None);
    }

    #[test]
    #[should_panic(expected = "The amount should be a positive number")]
    fn batch_ft_transfer_rejects_zero_amount() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(0))], None);
    }

    #[test]
    #[should_panic(expected = "The account sweat_user1 is restricted")]
    fn batch_ft_transfer_rejects_denylisted_sender() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        as_caller(token());
        contract.set_restricted(&user1(), true);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(1))], None);
    }

    #[test]
    #[should_panic(expected = "The account sweat_user3 is restricted")]
    fn batch_ft_transfer_rejects_denylisted_receiver() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        as_caller(token());
        contract.set_restricted(&user3(), true);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(1)), (user3(), U128(1))], None);
    }

    #[test]
    #[should_panic(expected = "Feature 'token' is paused")]
    fn batch_ft_transfer_blocked_when_token_paused() {
        let mut contract = setup();
        deposit(&mut contract, &user1(), 100);
        as_caller(token());
        contract.pause_features(vec![Feature::Token]);

        as_sender(user1());
        contract.batch_ft_transfer(vec![(user2(), U128(1))], None);
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
