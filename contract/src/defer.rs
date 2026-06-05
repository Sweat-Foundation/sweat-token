//! Deferred minting via the holding account.
//!
//! Newly minted user rewards are **not** credited to end-user accounts at mint
//! time. End users must **actively claim** their rewards from a separate
//! protocol-owned contract — the *holding account* — which is the only party
//! (besides the Oracle fee) that this contract ever mints user-portion tokens
//! to.
//!
//! # Roles
//!
//! - **This contract** owns the supply and controls minting. It never keeps a
//!   per-user ledger of unclaimed rewards.
//! - **Holding account** ([`Contract::holding_account_id`]) is a separate,
//!   protocol-owned smart contract. It is trusted by this contract (which is
//!   why [`crate::core::InternalDeposit::internal_deposit`] is allowed to skip
//!   storage registration for it — see the doc-comment there). The holding
//!   contract maintains the per-user ledger of unclaimed rewards and releases
//!   tokens on claim.
//! - **End user** earns steps. After a batch is recorded, the user calls the
//!   holding contract to claim their accumulated balance; the holding contract
//!   then performs a standard NEP-141 `ft_transfer` from itself to the user.
//!
//! # Batch lifecycle: from steps to a claimable balance
//!
//! 1. An [`Role::Oracle`] grantee calls [`SweatDefer::defer_batch`] with a
//!    `Vec<(AccountId, steps)>`.
//! 2. This contract computes `(amount, fee)` per `(user, steps)` pair, sums
//!    them, advances `steps_since_tge`, and packs the per-user breakdown into
//!    `amounts: Vec<(AccountId, U128)>`.
//! 3. It calls `record_batch_for_hold({ amounts })` on the holding account.
//!    This is the hand-off point: the holding account persists the per-user
//!    accounting (so it knows exactly how much each user is owed) *before* any
//!    tokens exist.
//! 4. The [`FungibleTokenTransferCallback::on_record`] callback:
//!    - **on success** mints `total_effective` (the user portion) to
//!      [`Contract::holding_account_id`] and `total_fee` to the Oracle that
//!      submitted the batch, and emits two [`FtMint`] events. The user portion
//!      now sits in the holding account's NEP-141 balance on this contract,
//!      earmarked in the holding contract's own storage for the individual
//!      users recorded in step 3.
//!    - **on failure** rolls back `steps_since_tge` and mints nothing. The
//!      holding account is expected to treat its own `record_batch_for_hold`
//!      call as transactional and not credit users for a batch whose mint did
//!      not land.
//! 5. Later, an end user calls a claim method on the **holding contract** (out
//!    of scope of this crate — it is a separately deployed contract). The
//!    holding contract decrements that user's internal ledger and issues a
//!    standard `ft_transfer` from `holding_account_id` to the user.
//!
//! # Invariants
//!
//! - `ft_balance_of(holding_account_id)` represents the pool of unclaimed user
//!   rewards plus any residual rounding dust. The holding account custodies
//!   this balance for users; it does not own it economically.
//! - There is no path on this contract by which a user receives newly minted
//!   rewards directly. Every reward path for end users goes through the
//!   holding contract's claim flow, which uses ordinary NEP-141 transfers and
//!   is therefore subject to the standard denylist and pause checks in
//!   [`crate::core`].
//! - This contract has no visibility into per-user pending balances. "What
//!   does Alice still have unclaimed?" must be answered by reading the holding
//!   contract's state, not this one.
//! - The trust assumption is explicit: a compromised or buggy holding contract
//!   could mis-credit users or fail to release claims, but it cannot mint
//!   additional supply — minting authority remains with the Oracle role and is
//!   gated by [`SweatDefer::defer_batch`].

use near_contract_standards::fungible_token::events::FtMint;
use near_sdk::{
    env::{self, panic_str},
    ext_contract, is_promise_success,
    json_types::U128,
    near, require,
    serde_json::json,
    AccountId, Gas, NearToken, Promise, PromiseOrValue,
};

use crate::{
    api::{RestrictionApi, SweatDefer},
    core::InternalDeposit,
    Contract, ContractExt, Feature, Role,
};
use near_plugins::{access_control_any, AccessControllable};

const GAS_FOR_DEFER_CALLBACK: Gas = Gas::from_tgas(5);
const GAS_FOR_DEFER: Gas = Gas::from_tgas(30);

#[near]
impl SweatDefer for Contract {
    #[access_control_any(roles(Role::Oracle))]
    fn defer_batch(&mut self, steps_batch: Vec<(AccountId, u32)>) -> PromiseOrValue<()> {
        self.assert_feature_enabled(Feature::Minting);
        require!(
            env::prepaid_gas() > GAS_FOR_DEFER,
            "Not enough gas to complete the operation"
        );

        let holding_account_id = self.holding_account_id.clone();

        let mut accounts_tokens: Vec<(AccountId, U128)> = Vec::new();
        let mut total_effective: U128 = U128(0);
        let mut total_fee: U128 = U128(0);
        let mut steps_increment: u64 = 0;

        for (account_id, step_count) in steps_batch {
            if self.is_restricted(&account_id) {
                continue;
            }

            let (amount, fee) = self.calculate_tokens_amount(step_count);
            self.steps_since_tge.0 += u64::from(step_count);
            steps_increment += u64::from(step_count);

            accounts_tokens.push((account_id, U128(amount)));
            total_effective.0 += amount;
            total_fee.0 += fee;
        }

        let hold_arguments = json!({
            "amounts": accounts_tokens,
        });

        let record_batch_for_hold_gas = env::prepaid_gas()
            .checked_sub(GAS_FOR_DEFER)
            .unwrap_or_else(|| panic_str("Prepaid gas overflow"));

        Promise::new(holding_account_id.clone())
            .function_call(
                "record_batch_for_hold".to_string(),
                hold_arguments.to_string().into_bytes(),
                NearToken::ZERO,
                record_batch_for_hold_gas,
            )
            .then(
                ext_ft_transfer_callback::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_DEFER_CALLBACK)
                    .on_record(
                        holding_account_id,
                        total_effective,
                        env::predecessor_account_id(),
                        total_fee,
                        steps_increment,
                    ),
            )
            .into()
    }
}

#[ext_contract(ext_ft_transfer_callback)]
pub trait FungibleTokenTransferCallback {
    fn on_record(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        fee_account_id: AccountId,
        fee: U128,
        steps_increment: u64,
    );
}

#[near]
impl FungibleTokenTransferCallback for Contract {
    #[private]
    fn on_record(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        fee_account_id: AccountId,
        fee: U128,
        steps_increment: u64,
    ) {
        if !is_promise_success() {
            self.steps_since_tge.0 -= steps_increment;
            env::log_str("Failed to record data in holding account; rolled back steps counter");

            return;
        }

        let mut events: Vec<FtMint> = Vec::with_capacity(2);

        self.internal_deposit(&fee_account_id, fee.0);
        events.push(FtMint {
            owner_id: &fee_account_id,
            amount: fee,
            memo: None,
        });

        self.internal_deposit(&receiver_id, amount.0);
        events.push(FtMint {
            owner_id: &receiver_id,
            amount,
            memo: None,
        });

        FtMint::emit_many(&events);
    }
}
