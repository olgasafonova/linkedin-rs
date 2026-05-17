//! Profile-shaped response models.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Lightweight profile used everywhere as an embedded reference.
///
/// Reference: `re/pegasus_models.md` -- `MiniProfile (voyager.identity.shared)`.
/// Appears inside `Me`, `Profile`, `Conversation` participants, etc.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniProfile {
    /// Entity URN, e.g. `urn:li:fs_miniProfile:ACoAABxxxxxx`.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,

    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,

    /// URL slug (vanity name), e.g. `john-doe-123`.
    #[serde(default)]
    pub public_identifier: Option<String>,

    /// Current role headline (labelled `occupation` in the API).
    #[serde(default)]
    pub occupation: Option<String>,

    /// Object URN, e.g. `urn:li:member:123456`.
    #[serde(default)]
    pub object_urn: Option<String>,

    /// Base64 tracking token.
    #[serde(default)]
    pub tracking_id: Option<String>,

    /// Profile photo.
    // TODO(live-validation): This is an Image union (VectorImage, MediaProxyImage,
    // URL string, MediaProcessorImage). Confirm union discriminator format --
    // FQN key like "com.linkedin.common.VectorImage" or short key "vectorImage".
    #[serde(default)]
    pub picture: Option<Value>,

    /// Background/banner image.
    // TODO(live-validation): Same Image union as picture. See above.
    #[serde(default)]
    pub background_image: Option<Value>,
}

/// Full profile with all details (~30+ fields).
///
/// Reference: `re/pegasus_models.md` -- `Profile (voyager.identity.profile)`.
/// Returned by `GET /voyager/api/identity/profiles/{id}` with decoration.
/// Fields kept as `Option` since we haven't validated all shapes against
/// the live API yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// First name.
    #[serde(default)]
    pub first_name: Option<String>,

    /// Last name.
    #[serde(default)]
    pub last_name: Option<String>,

    /// Maiden name.
    #[serde(default)]
    pub maiden_name: Option<String>,

    /// Headline (distinct from MiniProfile's `occupation`).
    #[serde(default)]
    pub headline: Option<String>,

    /// "About" section.
    #[serde(default)]
    pub summary: Option<String>,

    /// Industry name.
    #[serde(default)]
    pub industry_name: Option<String>,

    /// Industry URN.
    #[serde(default)]
    pub industry_urn: Option<String>,

    /// Location name (e.g. "San Francisco Bay Area").
    #[serde(default)]
    pub location_name: Option<String>,

    /// Geo location name.
    #[serde(default)]
    pub geo_location_name: Option<String>,

    /// Geo country name.
    #[serde(default)]
    pub geo_country_name: Option<String>,

    /// Geo country URN.
    #[serde(default)]
    pub geo_country_urn: Option<String>,

    /// Structured location.
    // TODO(live-validation): Type as ProfileLocation struct (countryCode, postalCode, etc.).
    #[serde(default)]
    pub location: Option<Value>,

    /// Structured geo location.
    // TODO(live-validation): Type as ProfileGeoLocation struct (geoUrn, etc.).
    #[serde(default)]
    pub geo_location: Option<Value>,

    /// Embedded mini profile reference.
    // TODO(live-validation): Type as MiniProfile once confirmed it's inlined (not a
    // URN reference in deduped responses). May need entity resolution if deduped.
    #[serde(default)]
    pub mini_profile: Option<Value>,

    /// Profile picture.
    // TODO(live-validation): Type as PhotoFilterPicture -- may wrap Image union with
    // an extra layer. Confirm actual nesting depth in live response.
    #[serde(default)]
    pub profile_picture: Option<Value>,

    /// Background image.
    // TODO(live-validation): May be BackgroundImage struct or direct Image union.
    #[serde(default)]
    pub background_image: Option<Value>,

    /// Whether this member is a student.
    #[serde(default)]
    pub student: Option<bool>,

    /// Version tag for optimistic concurrency.
    #[serde(default)]
    pub version_tag: Option<String>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Work experience entry.
///
/// Reference: `re/pegasus_models.md` -- `Position (voyager.identity.profile)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// Job title.
    #[serde(default)]
    pub title: Option<String>,

    /// Company name.
    #[serde(default)]
    pub company_name: Option<String>,

    /// Company URN.
    #[serde(default)]
    pub company_urn: Option<String>,

    /// Structured company info.
    #[serde(default)]
    pub company: Option<Value>,

    /// Role description.
    #[serde(default)]
    pub description: Option<String>,

    /// Location name.
    #[serde(default)]
    pub location_name: Option<String>,

    /// Geo location name.
    #[serde(default)]
    pub geo_location_name: Option<String>,

    /// Geo URN.
    #[serde(default)]
    pub geo_urn: Option<String>,

    /// Time period (start/end dates).
    #[serde(default)]
    pub time_period: Option<Value>,

    /// Whether this is a promotion within the same company.
    #[serde(default)]
    pub promotion: Option<bool>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}

/// Education entry.
///
/// Reference: `re/pegasus_models.md` -- `Education (voyager.identity.profile)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Education {
    /// Entity URN.
    #[serde(default)]
    pub entity_urn: Option<String>,

    /// School name.
    #[serde(default)]
    pub school_name: Option<String>,

    /// School URN.
    #[serde(default)]
    pub school_urn: Option<String>,

    /// Structured school info.
    #[serde(default)]
    pub school: Option<Value>,

    /// Degree name.
    #[serde(default)]
    pub degree_name: Option<String>,

    /// Degree URN.
    #[serde(default)]
    pub degree_urn: Option<String>,

    /// Field of study.
    #[serde(default)]
    pub field_of_study: Option<String>,

    /// Field of study URN.
    #[serde(default)]
    pub field_of_study_urn: Option<String>,

    /// Description.
    #[serde(default)]
    pub description: Option<String>,

    /// Grade.
    #[serde(default)]
    pub grade: Option<String>,

    /// Activities.
    #[serde(default)]
    pub activities: Option<String>,

    /// Time period (start/end dates).
    #[serde(default)]
    pub time_period: Option<Value>,

    /// Catch-all for fields not explicitly modelled.
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, Value>>,
}
