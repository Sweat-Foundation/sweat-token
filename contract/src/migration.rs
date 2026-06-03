use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::{collections::UnorderedSet, env, json_types::U64, near, AccountId};

use crate::{Contract, ContractExt};

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
    pub fn migrate(
        holding_account_id: AccountId,
        super_admin_account_id: AccountId,
        denylist_manager_account_ids: Vec<AccountId>,
        pause_manager_account_ids: Vec<AccountId>,
        unpause_manager_account_ids: Vec<AccountId>,
    ) -> Self {
        let mut old: OldContract = env::state_read().expect("failed to read old contract state");

        let mut contract = Self {
            token: old.token,
            steps_since_tge: old.steps_since_tge,
            denylist: UnorderedSet::new(b"d"),
            holding_account_id,
        };

        let oracle_account_ids: Vec<AccountId> = old.oracles.iter().collect();
        old.oracles.clear();

        contract.init_acl(
            super_admin_account_id,
            oracle_account_ids,
            denylist_manager_account_ids,
            pause_manager_account_ids,
            unpause_manager_account_ids,
        );

        contract
    }
}
