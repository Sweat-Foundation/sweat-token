use near_sdk::{near_bindgen, AccountId};

use crate::{Contract, ContractExt};

#[near_bindgen]
impl Contract {
    #[private]
    pub fn settle(&mut self) {
        let double_funded_accounts: Vec<(&str, u128)> = vec![
            ("sweat", 1248411988140030276307780),
            (
                "a41e363d574212248a571359c4f7179f50017490ebf8c406055f65a921a89c49",
                1268671155171721700632942,
            ),
            (
                "b175214bb0007915df266d77fd4d5aca78235aecdf08221bc6eeb95a8c299ab4",
                1332978442986901962461356,
            ),
            (
                "086bce94f5548d40d9087a6a3b64cbdd7cbcd2af85408fdd6bad39316c2dbcef",
                1363811903289185431239862,
            ),
        ];

        let target = &AccountId::new_unchecked("hodl-lockup.sweat".to_string());
        for (account_id, amount) in double_funded_accounts {
            let account_id = AccountId::new_unchecked(account_id.to_string());
            self.token.internal_transfer(&account_id, &target, amount.into(), None);
        }
    }
}
