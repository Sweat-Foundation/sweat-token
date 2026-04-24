use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedSet,
    json_types::U64,
    near_bindgen, AccountId,
};

use crate::{Contract, ContractExt};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct OldContract {
    oracles: UnorderedSet<AccountId>,
    token: FungibleToken,
    steps_since_tge: U64,
}

#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    /// # Panics
    ///
    /// Panics if the old contract state cannot be read.
    pub fn migrate_state() -> Self {
        let accounts_to_deny = vec![
            "59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d",
            "2a09040428a403edfd6a238e3a35325cf72a101bb464dbf7668e8fb954618b4d",
            "293a4f9a6790ae9d4db4124f766b39a28133bb55809254b9aa6c004de2d82ef5",
            "aa2485badfef481c55d927685630953ffc4cbc6819ea20c4a736a9573e79a2c7",
        ];

        let old: OldContract = near_sdk::env::state_read().expect("Old state doesn't exist");
        let mut denylist: UnorderedSet<AccountId> = UnorderedSet::new(b"d");
        for account_id in accounts_to_deny {
            denylist.insert(&AccountId::new_unchecked(account_id.to_string()));
        }

        Self {
            oracles: old.oracles,
            token: old.token,
            steps_since_tge: old.steps_since_tge,
            denylist,
        }
    }
}
