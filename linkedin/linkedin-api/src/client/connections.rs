//! Connection-graph API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::{graphql_params, unwrap_graphql};
use super::LinkedInClient;

impl LinkedInClient {
    /// Fetch the user's connections (sorted RECENTLY_ADDED).
    pub async fn get_connections(&self, start: u32, count: u32) -> Result<Value, Error> {
        let path = format!(
            "relationships/connections?start={}&count={}&sortType=RECENTLY_ADDED",
            start, count
        );
        self.get(&path).await
    }

    /// Send a connection request (invitation) via the Dash
    /// MemberRelationships `verifyQuotaAndCreateV2` action.
    pub async fn send_connection_request(
        &self,
        profile_urn: &str,
        message: Option<&str>,
    ) -> Result<Value, Error> {
        let mut payload = serde_json::json!({
            "invitee": {
                "inviteeUnion": {
                    "memberProfile": profile_urn
                }
            }
        });
        if let Some(msg) = message {
            payload
                .as_object_mut()
                .unwrap()
                .insert("message".to_string(), Value::String(msg.to_string()));
        }
        let path = "voyagerRelationshipsDashMemberRelationships\
                    ?action=verifyQuotaAndCreateV2\
                    &decorationId=com.linkedin.voyager.dash.deco.relationships.InvitationCreationResultWithInvitee-2";
        self.post(path, &payload).await
    }

    /// Fetch pending (received) connection invitations.
    pub async fn get_invitations(&self, start: u32, count: u32) -> Result<Value, Error> {
        let variables = format!("(start:{},count:{},includeInsights:true)", start, count);
        let params = graphql_params(
            &variables,
            "voyagerRelationshipsDashInvitationViews.48949225027e0a85d063176777f08e7f",
            "ReceivedInvitationViews",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "relationshipsDashInvitationViewsByReceived")
    }

    /// Accept a pending connection invitation.
    pub async fn accept_invitation(
        &self,
        invitation_urn: &str,
        shared_secret: &str,
    ) -> Result<Value, Error> {
        self.invitation_action(invitation_urn, shared_secret, "accept")
            .await
    }

    /// Withdraw a sent (pending) connection invitation.
    pub async fn withdraw_invitation(
        &self,
        invitation_urn: &str,
        shared_secret: &str,
    ) -> Result<Value, Error> {
        self.invitation_action(invitation_urn, shared_secret, "withdraw")
            .await
    }

    /// Shared body for accept/withdraw: identical endpoint shape, only the
    /// action segment differs.
    async fn invitation_action(
        &self,
        invitation_urn: &str,
        shared_secret: &str,
        action: &str,
    ) -> Result<Value, Error> {
        let path = format!(
            "voyagerRelationshipsDashInvitations/{}?action={}",
            invitation_urn, action
        );
        let body = serde_json::json!({
            "invitationUrn": invitation_urn,
            "sharedSecret": shared_secret
        });
        self.post(&path, &body).await
    }
}
