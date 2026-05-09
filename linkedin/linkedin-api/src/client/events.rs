//! Events API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::unwrap_graphql;
use super::LinkedInClient;

impl LinkedInClient {
    /// Fetch event details by numeric event ID.
    pub async fn get_event(&self, event_id: &str) -> Result<Value, Error> {
        let variables = format!(
            "(eventIdentifier:{event_id},includeComments:true,includeCommentsFirstReply:true)"
        );
        let params = format!(
            "includeWebMetadata=true&variables={}&queryId=voyagerEventsDashProfessionalEvents.023bf00e3eb9398c5fd7bb7665b0762d",
            variables
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "eventsDashProfessionalEventsByEventIdentifier")
    }

    /// Fetch attendees of a LinkedIn event.
    pub async fn get_event_attendees(
        &self,
        event_id: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let path = format!(
            "search/dash/clusters\
             ?decorationId=com.linkedin.voyager.dash.deco.search.SearchClusterCollection-174\
             &origin=EVENT_PAGE_CANNED_SEARCH\
             &q=all\
             &query=(flagshipSearchIntent:SEARCH_SRP,\
             queryParameters:(eventAttending:List({event_id}),\
             resultType:List(PEOPLE)),\
             includeFiltersInResponse:true)\
             &start={start}&count={count}"
        );
        self.get(&path).await
    }
}
