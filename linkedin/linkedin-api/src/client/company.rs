//! Company API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::restli_encode_string;
use super::LinkedInClient;

impl LinkedInClient {
    /// Fetch followers of a company page. Tries multiple endpoint patterns
    /// since the available paths vary by API version and admin status.
    pub async fn get_company_followers(
        &self,
        company_id: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let paths = [
            format!(
                "organization/organizationFollowerStatistics?q=organizationUrn&organizationUrn=urn%3Ali%3Aorganization%3A{}&start={}&count={}",
                company_id, start, count
            ),
            format!(
                "voyagerOrganizationCompanies/urn:li:fs_normalized_company:{}/followers?start={}&count={}",
                company_id, start, count
            ),
            format!(
                "organization/companies/{}/followers?start={}&count={}",
                company_id, start, count
            ),
        ];

        let mut last_err = Error::Api {
            status: 0,
            body: "no follower endpoints succeeded".to_string(),
            correlation_id: None,
        };
        for path in &paths {
            match self.get(path).await {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// Fetch company/organization info by universal name (URL slug).
    /// Falls back to `voyagerEntitiesCompanies` if the decorated endpoint
    /// fails.
    pub async fn get_company(&self, universal_name: &str) -> Result<Value, Error> {
        let path = format!(
            "organization/companies?q=universalName&universalName={}",
            restli_encode_string(universal_name)
        );
        if let Ok(resp) = self.get(&path).await {
            return Ok(first_or_self(resp));
        }
        let path2 = format!(
            "voyagerEntitiesCompanies?q=universalName&universalName={}",
            restli_encode_string(universal_name)
        );
        let resp = self.get(&path2).await?;
        Ok(first_or_self(resp))
    }
}

/// If `resp` is a collection with `elements[0]`, return that; otherwise
/// return the value as-is.
fn first_or_self(resp: Value) -> Value {
    resp.get("elements")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(resp)
}
