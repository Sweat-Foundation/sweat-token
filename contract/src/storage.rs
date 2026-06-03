use near_contract_standards::{
    fungible_token::events::FtBurn,
    storage_management::{StorageBalance, StorageBalanceBounds, StorageManagement},
};
use near_plugins::{pause, Pausable};
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
    #[pause(name = "token")]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.assert_not_in_denylist(vec![&env::predecessor_account_id()]);

        self.token
            .internal_storage_unregister(force)
            .inspect(|(account_id, balance)| {
                FtBurn {
                    owner_id: &account_id,
                    amount: (*balance).into(),
                    memo: None,
                }
                .emit();
            })
            .is_some()
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}
