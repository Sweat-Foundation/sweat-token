use near_contract_standards::fungible_token::{Balance, FungibleToken};
use near_sdk::{
    collections::{LookupMap, UnorderedSet},
    env,
    json_types::U64,
    near, AccountId, StorageUsage,
};

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
    token: OldFungibleToken,
    steps_since_tge: U64,
    denylist: UnorderedSet<AccountId>,
}

/// Legacy `FungibleToken` layout from the deployed contract.
///
/// The current token's `accounts` adapter dropped a `skip_hashing_postfix:
/// Option<String>` field, which still sits in the deployed state blob between
/// the map prefix and `total_supply`. The new struct would mis-deserialize
/// those bytes, so the old state is read through this mirror first. Only the
/// scalar tallies are reused; the account entries stay in place on the trie
/// (their keys are unchanged) and remain addressable by the rebuilt token.
#[near(serializers = [borsh])]
#[allow(dead_code)]
struct OldFungibleToken {
    accounts: OldLookupMapAdapter,
    total_supply: Balance,
    account_storage_usage: StorageUsage,
}

#[near(serializers = [borsh])]
#[allow(dead_code)] // fields exist only to consume the legacy borsh layout
struct OldLookupMapAdapter {
    // `LookupMap` persists nothing but its key prefix, so the key type here is
    // irrelevant for deserialization — it only needs to satisfy borsh bounds.
    inner: LookupMap<[u8; 32], Balance>,
    skip_hashing_postfix: Option<String>,
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
            token: FungibleToken::from_prefix(b"t", old.token.total_supply, old.token.account_storage_usage),
            steps_since_tge: old.steps_since_tge,
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
