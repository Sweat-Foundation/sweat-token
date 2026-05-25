#[macro_use]
extern crate static_assertions;

use api::{Payout, RestrictionApi, SweatApi};
use near_contract_standards::fungible_token::{
    events::{FtBurn, FtMint},
    metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider},
    Balance, FungibleToken,
};
use near_plugins::{access_control, access_control_any, pause, AccessControlRole, AccessControllable, Pausable};
use near_sdk::{
    borsh::BorshDeserialize,
    collections::UnorderedSet,
    env,
    json_types::{U128, U64},
    near, require,
    serde::{Deserialize, Serialize},
    AccountId, PanicOnDefault,
};

mod api;
mod core;
mod defer;
mod integration;
mod math;
mod migration;

#[derive(AccessControlRole, Deserialize, Serialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub enum Role {
    Oracle,
    PauseManager,
    UnpauseManager,
}

#[near(contract_state)]
#[access_control(role_type(Role))]
#[derive(Pausable, PanicOnDefault)]
#[pausable(pause_roles(Role::PauseManager), unpause_roles(Role::UnpauseManager))]
pub struct Contract {
    token: FungibleToken,
    steps_since_tge: U64,
    denylist: UnorderedSet<AccountId>,
    /// The single trusted account that `defer_batch` stages deferred mints
    /// through. Set by the super admin; never supplied by the Oracle.
    holding_account_id: Option<AccountId>,
}

#[near]
impl SweatApi for Contract {
    #[init]
    fn new(postfix: Option<String>, holding_account_id: Option<AccountId>) -> Self {
        let mut contract = Self {
            token: FungibleToken::new(b"t", postfix),
            steps_since_tge: U64::from(0),
            denylist: UnorderedSet::new(b"d"),
            holding_account_id,
        };

        contract.acl_init_super_admin(env::current_account_id());

        contract
    }

    #[private]
    fn add_oracle(&mut self, account_id: &AccountId) {
        self.acl_grant_role(Role::Oracle.into(), account_id.clone());
    }

    #[private]
    fn remove_oracle(&mut self, account_id: &AccountId) {
        self.acl_revoke_role(Role::Oracle.into(), account_id.clone());
    }

    #[private]
    fn set_holding_account_id(&mut self, account_id: AccountId) {
        self.holding_account_id = Some(account_id);
    }

    fn get_holding_account_id(&self) -> Option<AccountId> {
        self.holding_account_id.clone()
    }

    #[pause(name = "token")]
    fn burn(&mut self, amount: U128) {
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

    #[access_control_any(roles(Role::Oracle))]
    #[pause(name = "minting")]
    fn record_batch(&mut self, steps_batch: Vec<(AccountId, u32)>) {
        let mut oracle_fee: U128 = U128(0);
        let mut sweats: Vec<U128> = Vec::with_capacity(steps_batch.len() + 1);
        let mut events = Vec::with_capacity(steps_batch.len() + 1);

        for (account_id, steps_count) in &steps_batch {
            let (minted_to_user, trx_oracle_fee) = self.calculate_tokens_amount(*steps_count);
            oracle_fee.0 += trx_oracle_fee;
            internal_deposit(&mut self.token, account_id, minted_to_user);

            sweats.push(U128(minted_to_user));
            self.steps_since_tge.0 += u64::from(*steps_count);
        }
        for i in 0..steps_batch.len() {
            events.push(FtMint {
                owner_id: &steps_batch[i].0,
                amount: sweats[i],
                memo: None,
            });
        }

        internal_deposit(&mut self.token, &env::predecessor_account_id(), oracle_fee.0);
        let oracle_event = FtMint {
            owner_id: &env::predecessor_account_id(),
            amount: oracle_fee,
            memo: None,
        };
        events.push(oracle_event);
        FtMint::emit_many(events.as_slice());
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

    #[private]
    fn set_restricted(&mut self, account_id: &AccountId, is_restricted: bool) {
        if is_restricted {
            self.denylist.insert(account_id);
        } else {
            self.denylist.remove(account_id);
        }
    }
}

impl Contract {
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

/// Taken from contract standards but modified to default if account isn't initialized
/// rather than panicking:
/// <https://github.com/near/near-sdk-rs/blob/6596dc311036fe51d94358ac8f6497ef6e5a7cfc/near-contract-standards/src/fungible_token/core_impl.rs#L105>
fn internal_deposit(token: &mut FungibleToken, account_id: &AccountId, amount: Balance) {
    let balance = token.accounts.get(account_id).unwrap_or_default();
    let new_balance = balance
        .checked_add(amount)
        .unwrap_or_else(|| env::panic_str("Balance overflow"));
    token.accounts.insert(account_id, &new_balance);
    token.total_supply = token
        .total_supply
        .checked_add(amount)
        .unwrap_or_else(|| env::panic_str("Total supply overflow"));
}

pub const ICON: &str = "data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Crect width='100' height='100' rx='50' fill='%23FF0D75'/%3E%3Cg clip-path='url(%23clip0_283_2788)'%3E%3Cpath d='M39.4653 77.5455L19.0089 40.02L35.5411 22.2805L55.9975 59.806L39.4653 77.5455Z' stroke='white' stroke-width='10'/%3E%3Cpath d='M66.0253 77.8531L45.569 40.3276L62.1012 22.5882L82.5576 60.1136L66.0253 77.8531Z' stroke='white' stroke-width='10'/%3E%3C/g%3E%3Cdefs%3E%3CclipPath id='clip0_283_2788'%3E%3Crect width='100' height='56' fill='white' transform='translate(0 22)'/%3E%3C/clipPath%3E%3C/defs%3E%3C/svg%3E%0A";

#[near]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: "ft-1.0".to_string(),
            name: "SWEAT".to_string(),
            symbol: "SWEAT".to_string(),
            icon: Some(String::from(ICON)),
            reference: None,
            reference_hash: None,
            decimals: 18,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use near_contract_standards::fungible_token::core::FungibleTokenCore;
    use near_plugins::AccessControllable;
    use near_sdk::{
        json_types::{U128, U64},
        test_utils::VMContextBuilder,
        testing_env, AccountId, NearToken,
    };

    use crate::{
        api::{RestrictionApi, SweatApi},
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
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        assert_eq!(vec![sweat_oracle()], get_oracles(&token));
        token.remove_oracle(&sweat_oracle());
        assert!(get_oracles(&token).is_empty());
    }

    #[test]
    fn oracle_fee_test() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert_eq!(U64(0), token.get_steps_since_tge());
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        assert_eq!(vec![sweat_oracle()], get_oracles(&token));
        testing_env!(get_context(sweat_the_token(), sweat_oracle()).build());
        token.record_batch(vec![(user1(), 10_000), (user2(), 10_000)]);
        assert!((9.499_999_991_723_028 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);
        assert!((9.499_999_975_169_082 - token.token.ft_balance_of(user2()).0 as f64 / 1e+18).abs() < EPS);
        assert!((0.999_999_998_257_479_4 - token.token.ft_balance_of(sweat_oracle()).0 as f64 / 1e+18).abs() < EPS);
        assert_eq!(U64(2 * 10_000), token.get_steps_since_tge());
    }

    #[test]
    fn burn() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.burn(U128(9499999991723028480));
        assert!((0.0 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is not registered"#)]
    fn transfer_to_unregistered() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        testing_env!(get_context(sweat_the_token(), user1()).build());

        token.token.ft_transfer(user2(), U128(9499999991723028480), None);

        assert!((0.0 - token.token.ft_balance_of(user1()).0 as f64 / 1e+18).abs() < EPS);

        assert!((9.499_999_991_723_028 - token.token.ft_balance_of(user2()).0 as f64 / 1e+18).abs() < EPS);
    }

    #[test]
    fn transfer_to_registered() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
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
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.set_restricted(&user1(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer(user2(), U128(9499999991723028480), None);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is restricted"#)]
    fn transfer_to_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.set_restricted(&user2(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer(user2(), U128(9499999991723028480), None);
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user1 is restricted"#)]
    fn ft_transfer_call_from_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.set_restricted(&user1(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer_call(user2(), U128(9499999991723028480), None, String::from("test"));
    }

    #[test]
    #[should_panic(expected = r#"The account sweat_user2 is restricted"#)]
    fn ft_transfer_call_to_denied_account() {
        testing_env!(get_context(sweat_the_token(), sweat_the_token()).build());
        let mut token = Contract::new(Some(".u.sweat".to_string()), None);
        assert!(get_oracles(&token).is_empty());
        token.add_oracle(&sweat_oracle());
        mint(&mut token, &user1(), 9499999991723028480);
        mint(&mut token, &user2(), 9499999991723028480);
        token.set_restricted(&user2(), true);

        testing_env!(get_context(sweat_the_token(), user1()).build());
        token.ft_transfer_call(user2(), U128(9499999991723028480), None, String::from("test"));
    }

    fn get_oracles(contract: &Contract) -> Vec<AccountId> {
        contract.acl_get_grantees(Role::Oracle.into(), 0, 10)
    }

    fn mint(contract: &mut Contract, account_id: &AccountId, amount: u128) {
        crate::internal_deposit(&mut contract.token, account_id, amount);
    }
}
