//! Search API methods on `LinkedInClient`.

use serde_json::Value;

use crate::error::Error;

use super::internal::{graphql_params, restli_encode_string, unwrap_graphql};
use super::LinkedInClient;

impl LinkedInClient {
    /// Search for people by keywords.
    pub async fn search_people(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        self.cluster_search(keywords, start, count, "PEOPLE").await
    }

    /// Search for content/posts by keywords.
    pub async fn search_content(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        self.cluster_search(keywords, start, count, "CONTENT").await
    }

    /// Shared GraphQL search across `PEOPLE` and `CONTENT` result types.
    /// Both share the `searchDashClustersByAll` finder; only the
    /// `resultType` enum value differs.
    async fn cluster_search(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
        result_type: &str,
    ) -> Result<Value, Error> {
        let restli_keywords = restli_encode_string(keywords);
        let variables = format!(
            "(count:{count},origin:GLOBAL_SEARCH_HEADER,query:(flagshipSearchIntent:SEARCH_SRP,keywords:{restli_keywords},queryParameters:(resultType:List({result_type}))),start:{start})"
        );
        let params = graphql_params(
            &variables,
            "voyagerSearchDashClusters.fae19421cdd51a7cd735e0b7d7b32e0f",
            "SearchClusterCollection",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "searchDashClustersByAll")
    }

    /// Search for jobs by keywords. Uses the dedicated
    /// `jobsDashJobCardsByJobSearch` finder; the general clusters finder
    /// returns 501 for JOBS.
    pub async fn search_jobs(
        &self,
        keywords: &str,
        start: u32,
        count: u32,
    ) -> Result<Value, Error> {
        let restli_keywords = restli_encode_string(keywords);
        let variables = format!(
            "(count:{count},includeJobState:true,query:(keywords:{restli_keywords},origin:FACETED_SEARCH),start:{start})"
        );
        let params = graphql_params(
            &variables,
            "voyagerJobsDashJobCards.4ef915ad5827cd8ea1351ad72f8e4268",
            "JobCardsByJobSearch",
        );
        let raw = self.graphql_get(&params).await?;
        unwrap_graphql(&raw, "jobsDashJobCardsByJobSearch")
    }
}
