//! Home-made per-feature pause mechanism.
//!
//! Paused features are stored as a bitmask on [`Contract::paused_features`].
//! Each [`Feature`] owns one bit (see [`Feature::bit`]). Pausing a feature ORs
//! its bit in; unpausing clears it. Guarded methods call
//! [`Contract::assert_feature_enabled`] as their first statement so the pause
//! check fires before any other validation.
//!
//! Pause/unpause are gated by the [`crate::Role::PauseManager`] /
//! [`crate::Role::UnpauseManager`] ACL roles and emit a
//! [`Event::FeaturePauseChanged`] for every feature whose state actually
//! changes.

use near_plugins::{access_control_any, AccessControllable};
use near_sdk::{near, require};

use crate::{api::PauseApi, event::Event, Contract, ContractExt, Role};

/// A contract feature that can be independently paused.
///
/// Serialized in snake_case so call args read e.g.
/// `{"features": ["token", "minting"]}`.
#[near(serializers = [json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Token movement: `ft_transfer`, `ft_transfer_call`, `burn`,
    /// `storage_unregister`.
    Token,
    /// Reward minting: `defer_batch`.
    Minting,
}

impl Feature {
    /// Every feature, used to enumerate the bitmask for view methods.
    const ALL: [Feature; 2] = [Feature::Token, Feature::Minting];

    /// The single bit this feature occupies in [`Contract::paused_features`].
    const fn bit(self) -> u32 {
        match self {
            Feature::Token => 1 << 0,
            Feature::Minting => 1 << 1,
        }
    }

    /// Human-readable name used in the paused panic message.
    const fn name(self) -> &'static str {
        match self {
            Feature::Token => "token",
            Feature::Minting => "minting",
        }
    }
}

#[near]
impl PauseApi for Contract {
    #[access_control_any(roles(Role::PauseManager))]
    fn pause_features(&mut self, features: Vec<Feature>) -> bool {
        self.set_features_paused(features, true)
    }

    #[access_control_any(roles(Role::UnpauseManager))]
    fn unpause_features(&mut self, features: Vec<Feature>) -> bool {
        self.set_features_paused(features, false)
    }

    fn is_feature_paused(&self, feature: Feature) -> bool {
        self.paused_features & feature.bit() != 0
    }

    fn get_paused_features(&self) -> Vec<Feature> {
        Feature::ALL
            .into_iter()
            .filter(|feature| self.is_feature_paused(*feature))
            .collect()
    }
}

impl Contract {
    /// Panics if `feature` is currently paused. Call this as the first statement
    /// of any guarded method so it fires before other checks.
    pub(crate) fn assert_feature_enabled(&self, feature: Feature) {
        require!(
            !self.is_feature_paused(feature),
            format!("Feature '{}' is paused", feature.name())
        );
    }

    /// Sets the paused bit of each feature to `paused`, emitting a
    /// [`Event::FeaturePauseChanged`] only for features whose state actually
    /// changed. Returns whether the mask changed at all.
    fn set_features_paused(&mut self, features: Vec<Feature>, paused: bool) -> bool {
        let before = self.paused_features;

        for feature in features {
            let already = self.is_feature_paused(feature);
            if already == paused {
                continue;
            }

            if paused {
                self.paused_features |= feature.bit();
            } else {
                self.paused_features &= !feature.bit();
            }

            Event::FeaturePauseChanged { feature, paused }.emit();
        }

        self.paused_features != before
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use near_contract_standards::fungible_token::core::FungibleTokenCore;
    use near_plugins::AccessControllable;
    use near_sdk::{json_types::U128, test_utils::VMContextBuilder, testing_env, AccountId, NearToken};

    use crate::{
        api::{PauseApi, SweatApi},
        core::InternalDeposit,
        pause::Feature,
        Contract, Role,
    };

    fn account_id(account_id: &str) -> AccountId {
        AccountId::from_str(account_id).unwrap()
    }

    fn token() -> AccountId {
        account_id("sweat_the_token")
    }
    fn holding() -> AccountId {
        account_id("sweat_the_holding")
    }
    fn user1() -> AccountId {
        account_id("sweat_user1")
    }
    fn user2() -> AccountId {
        account_id("sweat_user2")
    }

    fn get_context(sender: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(token())
            .signer_account_id(sender.clone())
            .predecessor_account_id(sender)
            .attached_deposit(NearToken::from_yoctonear(1));
        builder
    }

    /// super-admin is `token()`, which can pause/unpause via ACL.
    fn setup() -> Contract {
        testing_env!(get_context(token()).build());
        let mut contract = Contract::new(holding(), token(), vec![], vec![], vec![], vec![]);
        contract.acl_grant_role(Role::PauseManager.into(), token());
        contract.acl_grant_role(Role::UnpauseManager.into(), token());
        contract
    }

    #[test]
    fn pause_then_unpause_round_trip() {
        let mut contract = setup();

        assert!(contract.get_paused_features().is_empty());

        assert!(contract.pause_features(vec![Feature::Token]));
        assert!(contract.is_feature_paused(Feature::Token));
        assert!(!contract.is_feature_paused(Feature::Minting));
        assert_eq!(contract.get_paused_features(), vec![Feature::Token]);

        assert!(contract.unpause_features(vec![Feature::Token]));
        assert!(!contract.is_feature_paused(Feature::Token));
        assert!(contract.get_paused_features().is_empty());
    }

    #[test]
    fn pause_is_idempotent() {
        let mut contract = setup();

        assert!(contract.pause_features(vec![Feature::Token]));
        // Already paused -> mask unchanged.
        assert!(!contract.pause_features(vec![Feature::Token]));

        assert!(contract.unpause_features(vec![Feature::Token]));
        // Already unpaused -> mask unchanged.
        assert!(!contract.unpause_features(vec![Feature::Token]));
    }

    #[test]
    fn pause_multiple_features_at_once() {
        let mut contract = setup();

        assert!(contract.pause_features(vec![Feature::Token, Feature::Minting]));
        assert!(contract.is_feature_paused(Feature::Token));
        assert!(contract.is_feature_paused(Feature::Minting));

        // One already paused, one not -> still a change.
        assert!(contract.unpause_features(vec![Feature::Token]));
        assert!(contract.unpause_features(vec![Feature::Token, Feature::Minting]));
        assert!(contract.get_paused_features().is_empty());
    }

    #[test]
    #[should_panic(expected = "Feature 'token' is paused")]
    fn ft_transfer_blocked_when_token_paused() {
        let mut contract = setup();
        contract.internal_deposit(&user1(), 9_000_000_000_000_000_000);
        contract.pause_features(vec![Feature::Token]);

        testing_env!(get_context(user1()).build());
        contract.ft_transfer(user2(), U128(1), None);
    }

    #[test]
    #[should_panic(expected = "Feature 'token' is paused")]
    fn burn_blocked_when_token_paused() {
        let mut contract = setup();
        contract.internal_deposit(&user1(), 9_000_000_000_000_000_000);
        contract.pause_features(vec![Feature::Token]);

        testing_env!(get_context(user1()).build());
        contract.burn(U128(1));
    }

    #[test]
    fn ft_transfer_works_after_unpause() {
        let mut contract = setup();
        contract.internal_deposit(&user1(), 9_000_000_000_000_000_000);
        contract.internal_deposit(&user2(), 9_000_000_000_000_000_000);

        contract.pause_features(vec![Feature::Token]);
        contract.unpause_features(vec![Feature::Token]);

        testing_env!(get_context(user1()).build());
        contract.ft_transfer(user2(), U128(1), None);
        assert_eq!(contract.ft_balance_of(user2()).0, 9_000_000_000_000_000_001);
    }

    #[test]
    #[should_panic(expected = "Insufficient permissions for method pause_features")]
    fn pause_requires_role() {
        testing_env!(get_context(token()).build());
        let mut contract = Contract::new(holding(), token(), vec![], vec![], vec![], vec![]);

        testing_env!(get_context(user1()).build());
        contract.pause_features(vec![Feature::Token]);
    }
}
