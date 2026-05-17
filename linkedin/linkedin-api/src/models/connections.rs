//! Connections-shaped response models.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Paging;

/// Top-level response from the `relationships/connections` endpoint.
///
/// Wraps a standard Rest.li collection of `Connection` items.
/// See `re/api_endpoint_catalog.md` section 8 and `re/pegasus_models.md`
/// for the `Connection` model definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsResponse {
    /// Array of connection items.
    #[serde(default)]
    pub elements: Vec<Value>,

    /// Pagination metadata for this page of results.
    #[serde(default)]
    pub paging: Option<Paging>,

    /// Collection-level metadata (type varies by endpoint).
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// A connection (1st-degree network member).
///
/// Reference: `re/pegasus_models.md` -- `Connection (voyager.relationships.shared.connection)`.
/// Fields kept as `Option` since we haven't validated against live API yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// URN identifying this connection, e.g. `urn:li:fs_connection:ACoAABxxxxxx`.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Embedded mini profile of the connected member.
    // TODO(live-validation): Type as MiniProfile once confirmed inlined (not URN ref).
    #[serde(default)]
    pub mini_profile: Option<Value>,

    /// Phone numbers shared by this connection.
    #[serde(default)]
    pub phone_numbers: Option<Vec<Value>>,

    /// Primary email address of this connection (if shared).
    #[serde(default)]
    pub primary_email_address: Option<String>,

    /// Twitter handles of this connection.
    #[serde(default)]
    pub twitter_handles: Option<Vec<Value>>,

    /// WeChat contact info.
    #[serde(default)]
    pub we_chat_contact_info: Option<Value>,

    /// Timestamp when the connection was established (epoch millis).
    #[serde(default)]
    pub created_at: Option<u64>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}
