//! SWEAT-specific contract events.
//!
//! These events follow the [events format (nep-297)] but, unlike the fungible
//! token events in `near-contract-standards`, they do not correspond to a NEP
//! standard. They are emitted under the project-specific `sweat` standard
//! namespace.
//!
//! [events format (nep-297)]: https://github.com/near/NEPs/blob/master/specs/Standards/EventsFormat.md
//!
//! To add a new event, add a variant to [`Event`]. Serde maps the variant name
//! (`snake_cased`) to the `event` field and its body to `data`; [`Event::emit`]
//! wraps it in the `standard`/`version` envelope and logs it.

use near_sdk::{env, serde::Serialize, serde_json, AccountIdRef};

use crate::Feature;

const STANDARD: &str = "sweat";
const VERSION: &str = "1.3.2";

/// A SWEAT contract event. The variant name (`snake_cased`) becomes the NEP-297
/// `event` discriminator and its fields the `data` payload.
#[must_use = "don't forget to `.emit()` this event"]
#[derive(Serialize, Debug)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum Event<'a> {
    /// Logged when an account is added to or removed from the denylist.
    RestrictionChanged {
        account_id: &'a AccountIdRef,
        /// The account's new restriction status: `true` if it was added to the
        /// denylist, `false` if it was removed.
        is_restricted: bool,
    },
    /// Logged when a [`Feature`] is paused or unpaused.
    FeaturePauseChanged {
        feature: Feature,
        /// The feature's new state: `true` if it was paused, `false` if it was
        /// unpaused.
        paused: bool,
    },
}

impl Event<'_> {
    /// Emits the event through [`env::log_str`](near_sdk::env::log_str). This is
    /// required to ensure that the event is triggered and to consume the event.
    pub fn emit(self) {
        let envelope = Envelope {
            standard: STANDARD,
            version: VERSION,
            event: self,
        };

        // Events cannot fail to serialize so fine to panic on error
        #[allow(clippy::redundant_closure)]
        let json = serde_json::to_string(&envelope).ok().unwrap_or_else(|| env::abort());
        env::log_str(&format!("EVENT_JSON:{json}"));
    }
}

/// The NEP-297 envelope. The flattened [`Event`] contributes the `event` and
/// `data` fields next to `standard` and `version`.
#[derive(Serialize, Debug)]
#[serde(crate = "near_sdk::serde")]
struct Envelope<'a> {
    standard: &'static str,
    version: &'static str,
    #[serde(flatten)]
    event: Event<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{test_utils, AccountIdRef};

    #[test]
    fn restriction_added() {
        let account_id = AccountIdRef::new_or_panic("bob");
        Event::RestrictionChanged {
            account_id,
            is_restricted: true,
        }
        .emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"sweat","version":"1.3.2","event":"restriction_changed","data":{"account_id":"bob","is_restricted":true}}"#
        );
    }

    #[test]
    fn restriction_removed() {
        let account_id = AccountIdRef::new_or_panic("bob");
        Event::RestrictionChanged {
            account_id,
            is_restricted: false,
        }
        .emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"sweat","version":"1.3.2","event":"restriction_changed","data":{"account_id":"bob","is_restricted":false}}"#
        );
    }

    #[test]
    fn feature_paused() {
        Event::FeaturePauseChanged {
            feature: Feature::Token,
            paused: true,
        }
        .emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"sweat","version":"1.3.2","event":"feature_pause_changed","data":{"feature":"token","paused":true}}"#
        );
    }

    #[test]
    fn feature_unpaused() {
        Event::FeaturePauseChanged {
            feature: Feature::Minting,
            paused: false,
        }
        .emit();
        assert_eq!(
            test_utils::get_logs()[0],
            r#"EVENT_JSON:{"standard":"sweat","version":"1.3.2","event":"feature_pause_changed","data":{"feature":"minting","paused":false}}"#
        );
    }
}
