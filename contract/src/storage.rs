use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds, StorageManagement};
use near_sdk::{env, near, AccountId, NearToken};

use crate::{Contract, ContractExt};

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    #[allow(clippy::used_underscore_binding)]
    fn storage_unregister(&mut self, _force: Option<bool>) -> bool {
        // Unregistering is intentionally disabled: the Sweat Foundation
        // subsidizes the storage of accounts created with Sweat Wallet, so we
        // don't allow users to unregister and reclaim their staked funds.
        env::panic_str("storage_unregister is disabled");
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}
