use near_contract_standards::fungible_token::core::FungibleTokenCore;
use near_sdk::{near_bindgen, AccountId};

use crate::{Contract, ContractExt};

#[near_bindgen]
impl Contract {
    #[private]
    pub fn restore_stolen_funds(&mut self) {
        // tx 2hTWy8fxsyP8BecomdM7WqSRDZSbY6ykNq2tapvFrtjZ
        // v2.jars.sweat -> 59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d
        // amount: 516915008662020000000000000

        // tx D4CgSta3QKr7qC6jXyv6ymHByEmt4cKvxeqdZKGN62rt
        // v2.jars.sweat -> 59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d
        // amount: 599996150965090000000000000

        // tx 21WcjEFdqAxgeR4AibwDubv7EvL7zKnfKWXvKjvwvE9P
        // v2.jars.sweat -> 59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d
        // amount: 100275327509797174551123618

        // tx HeJdK2iwk3jb2vrhkGBYppfLeMPWviX7E48ZvXV3Y4kf
        // jars.sweat -> 59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d
        // amount: 155211794370988073784465099

        let exploiter_account_id =
            AccountId::new_unchecked("59cf9840aa73006ba14ed99df798cb4a24a0d54e7cba0db749df9b6960dc872d".to_string());
        let victim_account_id = AccountId::new_unchecked("v2.jars.sweat".to_string());
        let amount = self.token.ft_balance_of(exploiter_account_id.clone());
        self.token
            .internal_transfer(&exploiter_account_id, &victim_account_id, amount.into(), None);
    }
}
