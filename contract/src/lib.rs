#[macro_use]
extern crate static_assertions;

use api::{Payout, RestrictionApi, SweatApi};
use near_contract_standards::fungible_token::{events::FtBurn, FungibleToken};
use near_plugins::{access_control, access_control_any, pause, AccessControlRole, AccessControllable, Pausable};
use near_sdk::{
    assert_one_yocto,
    borsh::BorshDeserialize,
    collections::UnorderedSet,
    env,
    json_types::{U128, U64},
    near, require,
    serde::{Deserialize, Serialize},
    AccountId, PanicOnDefault,
};

pub mod api;
mod core;
mod defer;
mod integration;
mod math;
mod meta;
mod migration;
mod storage;

#[derive(AccessControlRole, Deserialize, Serialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub enum Role {
    Oracle,
    PauseManager,
    UnpauseManager,
    DenylistManager,
}

#[near(contract_state)]
#[access_control(role_type(Role))]
#[derive(Pausable, PanicOnDefault)]
#[pausable(pause_roles(Role::PauseManager), unpause_roles(Role::UnpauseManager))]
pub struct Contract {
    token: FungibleToken,
    steps_since_tge: U64,
    denylist: UnorderedSet<AccountId>,
    /// Trusted protocol-owned contract that custodies the user portion of every
    /// minted batch until end users claim their rewards. See the [`defer`]
    /// module docs for the full claim flow and trust model.
    holding_account_id: AccountId,
}

#[near]
impl SweatApi for Contract {
    #[init]
    fn new(
        postfix: Option<String>,
        holding_account_id: AccountId,
        super_admin_account_id: AccountId,
        oracle_account_ids: Vec<AccountId>,
        denylist_manager_account_ids: Vec<AccountId>,
        pause_manager_account_ids: Vec<AccountId>,
        unpause_manager_account_ids: Vec<AccountId>,
    ) -> Self {
        let mut contract = Self {
            token: FungibleToken::new(b"t", postfix),
            steps_since_tge: U64::from(0),
            denylist: UnorderedSet::new(b"d"),
            holding_account_id,
        };

        contract.init_acl(
            super_admin_account_id,
            oracle_account_ids,
            denylist_manager_account_ids,
            pause_manager_account_ids,
            unpause_manager_account_ids,
        );

        contract
    }

    #[private]
    fn set_holding_account_id(&mut self, account_id: AccountId) {
        self.holding_account_id = account_id;
    }

    fn get_holding_account_id(&self) -> AccountId {
        self.holding_account_id.clone()
    }

    #[pause(name = "token")]
    #[payable]
    fn burn(&mut self, amount: U128) {
        assert_one_yocto();
        self.assert_not_in_denylist(vec![&env::predecessor_account_id()]);

        self.token.internal_withdraw(&env::predecessor_account_id(), amount.0);
        FtBurn {
            amount,
            owner_id: &env::predecessor_account_id(),
            memo: None,
        }
        .emit();
    }

    fn get_steps_since_tge(&self) -> U64 {
        self.steps_since_tge
    }

    #[allow(clippy::cast_precision_loss)]
    fn formula(&self, steps_since_tge: U64, steps: u32) -> U128 {
        U128(math::formula(steps_since_tge.0 as f64, f64::from(steps)))
    }
}

#[near]
impl RestrictionApi for Contract {
    fn is_restricted(&self, account_id: &AccountId) -> bool {
        self.denylist.contains(account_id)
    }

    #[access_control_any(roles(Role::DenylistManager))]
    fn set_restricted(&mut self, account_id: &AccountId, is_restricted: bool) {
        if is_restricted {
            self.denylist.insert(account_id);
        } else {
            self.denylist.remove(account_id);
        }
    }
}

impl Contract {
    /// Initializes the ACL: sets the super admin and grants the manager roles.
    ///
    /// Uses the unchecked grant path because the super admin is an arbitrary
    /// account rather than the caller, so the permission-checked
    /// `acl_grant_role` would silently reject these grants. This must only be
    /// called from `#[init]` or `#[private]` methods.
    fn init_acl(
        &mut self,
        super_admin_account_id: AccountId,
        oracle_account_ids: Vec<AccountId>,
        denylist_manager_account_ids: Vec<AccountId>,
        pause_manager_account_ids: Vec<AccountId>,
        unpause_manager_account_ids: Vec<AccountId>,
    ) {
        self.acl_init_super_admin(super_admin_account_id);

        let grants = [
            (Role::Oracle, oracle_account_ids),
            (Role::DenylistManager, denylist_manager_account_ids),
            (Role::PauseManager, pause_manager_account_ids),
            (Role::UnpauseManager, unpause_manager_account_ids),
        ];

        for (role, account_ids) in grants {
            for account_id in account_ids {
                self.acl_get_or_init().grant_role_unchecked(role, &account_id);
            }
        }
    }

    pub(crate) fn calculate_tokens_amount(&self, steps: u32) -> (u128, u128) {
        let sweat_to_mint: u128 = self.formula(self.steps_since_tge, steps).0;
        let payout = Payout::from(sweat_to_mint);

        (payout.amount_for_user, payout.fee)
    }

    pub(crate) fn assert_not_in_denylist(&self, account_ids: Vec<&AccountId>) {
        for account_id in account_ids {
            require!(
                !self.is_restricted(account_id),
                format!("The account {account_id} is restricted")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use near_contract_standards::fungible_token::core::FungibleTokenCore;
    use near_plugins::AccessControllable;
    use near_sdk::{
        json_types::{U128, U64},
        test_utils::VMContextBuilder,
        test_vm_config, testing_env, AccountId, NearToken, PromiseResult, RuntimeFeesConfig,
    };

    use crate::{
        api::{Payout, RestrictionApi, SweatApi, SweatDefer},
        core::InternalDeposit,
        defer::FungibleTokenTransferCallback,
        Contract, Role,
    };

    const EPS: f64 = 0.00001;

    fn account_id(account_id: &str) -> AccountId {
        AccountId::from_str(account_id).unwrap()
    }

    fn sweat_the_token() -> AccountId {
        account_id("sweat_the_token")
    }
    fn sweat_oracle() -> AccountId {
        account_id("sweat_the_oracle")
    }
    fn sweat_holding() -> AccountId {
        account_id("sweat_the_holding")
    }
    fn user1() -> AccountId {
        account_id("sweat_user1")
    }
    fn user2() -> AccountId {
        account_id("sweat_user2")
    }

    fn get_context(owner: AccountId, sender: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(owner.clone())
            .signer_account_id(sender.clone())
            .predecessor_account_id(sender)
            .attached_deposit(NearToken::from_yoctonear(1));

        builder
    }

    #[test]
    fn add_remove_oracle() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        assert_eq!(vec![sweat_oracle()], get_oracles(&token));
        token.acl_revoke_role(Role::Oracle.into(), sweat_oracle());
        assert!(get_oracles(&token).is_empty());
    }

    #[test]
    fn oracle_fee_test() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert_eq!(U64(0), token.get_steps_since_tge());
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        assert_eq!(vec![sweat_oracle()], get_oracles(&token));

        let p1 = Payout::from(token.formula(U64(0), 10_000).0);
        let p2 = Payout::from(token.formula(U64(10_000), 10_000).0);
        let total_effective = U128(p1.amount_for_user + p2.amount_for_user);
        let total_fee = U128(p1.fee + p2.fee);

        testing_env!(get_context(sweat_the_token(), sweat_oracle()).build());
        let _ = token.defer_batch(vec![(user1(), 10_000), (user2(), 10_000)]);
        assert_eq!(U64(2 * 10_000), token.get_steps_since_tge());

        testing_env!(
            get_context(sweat_the_token(), sweat_the_token()).build(),
            test_vm_config(),
            RuntimeFeesConfig::test(),
            HashMap::default(),
            vec![PromiseResult::Successful(vec![])],
        );
        token.on_record(sweat_holding(), total_effective, sweat_oracle(), total_fee, 2 * 10_000);

        assert_eq!(token.token.ft_balance_of(sweat_holding()).0, total_effective.0);
        assert_eq!(token.token.ft_balance_of(sweat_oracle()).0, total_fee.0);
        assert!(((9.499_999_991_723_028 + 9.499_999_975_169_082) - total_effective.0 as f64 / 1e+18).abs() < EPS);
        assert!((0.999_999_998_257_479_4 - total_fee.0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    fn defer_batch_skips_denylisted_user() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());

        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user1(), true);

        testing_env!(get_context(sweat_the_token(), sweat_oracle()).build());
        let _ = token.defer_batch(vec![(user1(), 10_000), (user2(), 10_000)]);

        // Only the non-denylisted user's steps are counted; user1 is skipped
        // before its step_count is added. steps_since_tge is the only on-chain
        // state defer_batch mutates here — the per-user amounts go out in the
        // record_batch_for_hold XCC args, which a unit test can't observe.
        assert_eq!(U64(10_000), token.get_steps_since_tge());
    }

    #[test]
    fn defer_batch_skips_all_denylisted_users() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());

        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user1(), true);
        token.set_restricted(&user2(), true);

        testing_env!(get_context(sweat_the_token(), sweat_oracle()).build());
        let _ = token.defer_batch(vec![(user1(), 10_000), (user2(), 10_000)]);

        // Every entry skipped -> no steps recorded.
        assert_eq!(U64(0), token.get_steps_since_tge());
    }

    #[test]
    fn burn() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.burn(U128(9499999991723028480));
        assert!((0.0 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    #[should_panic(expected = r#"Requires attached deposit of exactly 1 yoctoNEAR"#)]
    fn burn_without_deposit() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1())
            .attached_deposit(NearToken::from_yoctonear(0))
            .build());
        token.burn(U128(9499999991723028480));
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is not registered"#)]
    fn transfer_to_unregistered() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1()).build());

        token.token.ft_transfer(user2(), U128(9499999991723028480), None);

        assert!((0.0 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);

        assert!((9.499_999_991_723_028 - token.token.ft_balance_of(user2()).0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    fn transfer_to_registered() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1()).build());

        token.token.ft_transfer(user2(), U128(9499999991723028480), None);

        assert!((0.0 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);

        assert!((9.499_999_991_723_028 * 2.0 - token.token.ft_balance_of(user2()).0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user1 is restricted"#)]
    fn transfer_from_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user1(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer(user2(), U128(9499999991723028480), None);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is restricted"#)]
    fn transfer_to_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user2(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer(user2(), U128(9499999991723028480), None);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user1 is restricted"#)]
    fn ft_transfer_call_from_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user1(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        let _ = token.ft_transfer_call(user2(), U128(9499999991723028480), None, String::from("test"));
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is restricted"#)]
    fn ft_transfer_call_to_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(
            Some(".u.sweat".to_string()),
            sweat_holding(),
            sweat_the_token(),
            vec![],
            vec![],
            vec![],
            vec![],
        );
        assert!(get_oracles(&token).is_empty());
        token.acl_grant_role(Role::Oracle.into(), sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.acl_grant_role(Role::DenylistManager.into(), sweat_the_token());
        token.set_restricted(&user2(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        let _ = token.ft_transfer_call(user2(), U128(9499999991723028480), None, String::from("test"));
    }

    fn get_oracles(contract: &Contract) -> Vec<AccountId> {
        contract.acl_get_grantees(Role::Oracle.into(), 0, 10)
    }

    fn mint(contract: &mut Contract, account_id: &AccountId, amount: u128) {
        contract.internal_deposit(account_id, amount);
    }
}
