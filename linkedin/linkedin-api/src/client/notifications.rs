//! Notifications API method on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::{graphql_params, unwrap_graphql};
use super::LinkedInClient;

impl LinkedInClient {
    /// Fetch the user's notification cards.
    ///
    /// Uses the `voyagerIdentityDashNotificationCards` GraphQL finder; the
    /// legacy REST endpoint returns 404 on the international build.
    pub async fn get_notifications(&self, start: u32, count: u32) -> Result<Value, Error> {
        let variables = format!("(start:{},count:{})", start, count);
        let params = graphql_params(
            &variables,
            "voyagerIdentityDashNotificationCards.1a1ca07d1f7a6e1033fd88d5fd2da611",
            "NotificationsCardsByFilterVanityName",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "identityDashNotificationCardsByFilterVanityName")
    }
}
