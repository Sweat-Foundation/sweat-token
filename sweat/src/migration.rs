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
        let old: OldContract = near_sdk::env::state_read().expect("Old state doesn't exist");

        Self {
            oracles: old.oracles,
            token: old.token,
            steps_since_tge: old.steps_since_tge,
            denylist: UnorderedSet::new(b"d"),
        }
    }
}
