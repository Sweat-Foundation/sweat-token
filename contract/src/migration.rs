use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::{collections::UnorderedSet, env, json_types::U64, near, AccountId};

use crate::{Contract, ContractExt};

/// State layout of the currently deployed contract (pre-ACL, with denylist).
///
/// Borsh deserializes fields in declaration order, so this must mirror the
/// deployed struct exactly — including `denylist`, which the live contract
/// already stores. Omitting it leaves trailing bytes in the state blob and
/// makes `state_read` panic with "Cannot deserialize the contract state".
#[near(serializers = [borsh])]
struct OldContract {
    oracles: UnorderedSet<AccountId>,
    token: FungibleToken,
    steps_since_tge: U64,
    denylist: UnorderedSet<AccountId>,
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

        // Drain the legacy oracle set into the ACL Oracle role, then reclaim its
        // storage. Read it out before moving the other fields into the new struct.
        let oracle_account_ids: Vec<AccountId> = old.oracles.iter().collect();
        old.oracles.clear();

        let mut contract = Self {
            token: old.token,
            steps_since_tge: old.steps_since_tge,
            // Carry the existing denylist over untouched. It already uses the
            // `b"d"` prefix, matching the new contract, so the stored entries
            // remain addressable without rewriting storage.
            denylist: old.denylist,
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
}
