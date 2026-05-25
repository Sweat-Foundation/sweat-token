use near_contract_standards::fungible_token::FungibleToken;
use near_plugins::AccessControllable;
use near_sdk::{collections::UnorderedSet, env, json_types::U64, near, AccountId};

use crate::{Contract, ContractExt, Role};

#[near(serializers = [borsh])]
struct OldContract {
    oracles: UnorderedSet<AccountId>,
    token: FungibleToken,
    steps_since_tge: U64,
}

#[near]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        let mut old: OldContract = env::state_read().expect("failed to read old contract state");

        let mut contract = Self {
            token: old.token,
            steps_since_tge: old.steps_since_tge,
            denylist: UnorderedSet::new(b"d"),
            holding_account_id: None,
        };

        contract.acl_init_super_admin(env::current_account_id());

        for oracle in old.oracles.iter() {
            contract.acl_grant_role(Role::Oracle.into(), oracle);
        }

        old.oracles.clear();

        contract
    }
}
