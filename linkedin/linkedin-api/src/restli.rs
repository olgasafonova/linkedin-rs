//! Helpers for navigating LinkedIn's Rest.li polymorphic union responses.
//!
//! Voyager and Dash responses encode polymorphic types as JSON objects keyed
//! by the fully-qualified Pegasus typename. A feed `element` looks like:
//!
//! ```text
//! {
//!   "value": {
//!     "com.linkedin.voyager.feed.render.UpdateV2": { ... real fields ... }
//!   }
//! }
//! ```
//!
//! Walking these shapes by hand spreads the typename strings across the
//! codebase. This module centralizes the typename constants and provides
//! tiny unwrap helpers so each call site can ignore the wrapper plumbing.

use serde_json::Value;

/// Voyager feed update typename.
pub const UPDATE_V2_KEY: &str = "com.linkedin.voyager.feed.render.UpdateV2";

/// WVMP (Who Viewed My Profile) card typenames. The wvmpCards endpoint
/// returns nested unions: a `WvmpViewersCard` contains `insightCards`, each
/// of which contains a `WvmpSummaryInsightCard` with the per-viewer entries.
pub const WVMP_VIEWERS_CARD: &str = "com.linkedin.voyager.identity.me.wvmpOverview.WvmpViewersCard";
pub const WVMP_SUMMARY_CARD: &str =
    "com.linkedin.voyager.identity.me.wvmpOverview.WvmpSummaryInsightCard";

/// Per-viewer card variants returned in a `WvmpSummaryInsightCard.cards[]`.
pub const WVMP_PROFILE_VIEW: &str = "com.linkedin.voyager.identity.me.WvmpProfileViewCard";
pub const WVMP_PRIVATE_VIEWER: &str = "com.linkedin.voyager.identity.me.PrivateProfileViewer";
pub const WVMP_GENERIC_CARD: &str = "com.linkedin.voyager.identity.me.WvmpGenericCard";
pub const WVMP_ANON_CARD: &str = "com.linkedin.voyager.identity.me.WvmpAnonymousProfileViewCard";
pub const WVMP_PREMIUM_UPSELL: &str = "com.linkedin.voyager.identity.me.WvmpPremiumUpsellCard";

/// Viewer profile wrapper used inside `WvmpProfileViewCard.viewer`.
pub const FULL_PROFILE_VIEWER: &str = "com.linkedin.voyager.identity.me.FullProfileViewer";

/// Unwrap a Rest.li polymorphic union by direct typename lookup.
///
/// Returns `Some(&inner)` when `obj.<typename>` exists, otherwise `None`.
/// Use this when the union is keyed directly on the object — no `value`
/// wrapper.
pub fn unwrap_union<'a>(obj: &'a Value, typename: &str) -> Option<&'a Value> {
    obj.get(typename)
}

/// Unwrap a Rest.li union under the standard `value.<typename>` Voyager shape.
///
/// Returns `Some(&inner)` when `element.value.<typename>` exists, otherwise
/// `None`.
pub fn unwrap_value_union<'a>(element: &'a Value, typename: &str) -> Option<&'a Value> {
    element.get("value").and_then(|v| v.get(typename))
}

/// Unwrap a feed element's UpdateV2 wrapper.
///
/// Some endpoints return elements pre-unwrapped (the bare UpdateV2 shape) and
/// some wrap them in the `value.com.linkedin.voyager.feed.render.UpdateV2`
/// envelope. Always returns a usable inner reference; falls back to the input
/// element when the wrapper isn't present.
pub fn unwrap_update_v2(element: &Value) -> &Value {
    unwrap_value_union(element, UPDATE_V2_KEY).unwrap_or(element)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unwrap_union_finds_typename_key() {
        let obj = json!({"com.linkedin.test.Foo": {"x": 1}});
        let inner = unwrap_union(&obj, "com.linkedin.test.Foo").expect("must find");
        assert_eq!(inner.get("x").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn unwrap_union_returns_none_on_miss() {
        let obj = json!({"other": {"x": 1}});
        assert!(unwrap_union(&obj, "com.linkedin.test.Foo").is_none());
    }

    #[test]
    fn unwrap_value_union_walks_value_then_typename() {
        let el = json!({
            "value": {
                "com.linkedin.test.Foo": {"x": 42}
            }
        });
        let inner = unwrap_value_union(&el, "com.linkedin.test.Foo").expect("must find");
        assert_eq!(inner.get("x").and_then(|v| v.as_u64()), Some(42));
    }

    #[test]
    fn unwrap_update_v2_strips_wrapper() {
        let wrapped = json!({
            "value": {
                UPDATE_V2_KEY: {"commentary": "hi"}
            }
        });
        let inner = unwrap_update_v2(&wrapped);
        assert!(inner.get("commentary").is_some());
        assert!(inner.get("value").is_none());
    }

    #[test]
    fn unwrap_update_v2_returns_input_when_unwrapped() {
        let bare = json!({"commentary": "hi"});
        let inner = unwrap_update_v2(&bare);
        assert_eq!(inner.get("commentary").and_then(|v| v.as_str()), Some("hi"));
    }

    #[test]
    fn wvmp_typename_constants_match_voyager_shape() {
        // Smoke-test that the typename prefix is intact. The Voyager Rest.li
        // protocol keys everything under `com.linkedin.voyager.`; if a constant
        // drifts from that, every WVMP lookup silently returns None.
        for tn in [
            WVMP_VIEWERS_CARD,
            WVMP_SUMMARY_CARD,
            WVMP_PROFILE_VIEW,
            WVMP_PRIVATE_VIEWER,
            WVMP_GENERIC_CARD,
            WVMP_ANON_CARD,
            WVMP_PREMIUM_UPSELL,
            FULL_PROFILE_VIEWER,
        ] {
            assert!(
                tn.starts_with("com.linkedin.voyager.identity.me"),
                "{tn} must live under voyager.identity.me"
            );
        }
        assert!(UPDATE_V2_KEY.starts_with("com.linkedin.voyager.feed"));
    }
}
