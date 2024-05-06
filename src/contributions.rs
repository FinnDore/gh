use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Days;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

use crate::SharedState;

#[instrument(skip(state))]
pub async fn contributions(
    Path(user): Path<String>,
    State(state): State<SharedState>,
) -> Result<Json<Vec<ContributionDay>>, String> {
    info!(user);
    let query = r#"
        query($userName:String!) {
            user(login: $userName){
            contributionsCollection {
                contributionCalendar {
                totalContributions
                weeks {
                    contributionDays {
                        contributionCount
                        date
                        }
                    }
                }
            }
            }
        }
    "#;

    let variables = format!(
        r#"{{
            "userName": "{}"
        }}"#,
        state.user.clone().or(Some(user)).unwrap()
    );
    let github_token = &state.github_token;
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.github.com/graphql")
        .body(serde_json::json!({ "query": query, "variables": variables }).to_string())
        .header("Authorization", format!("Bearer {}", github_token))
        .header("user-agent", "rust uwu")
        .send()
        .await
        .unwrap();
    let body = res.text().await;

    if let Err(err) = body {
        error!(%err, "failed to convert body to text");
        return Err("internal".to_string());
    }

    let body = body.unwrap();

    let json_body = serde_json::from_str(&body);
    if let Err(err) = json_body {
        error!(%err, "failed to parse body");
        return Err("internal".to_string());
    }

    let parsed_body: GithubContributionsResponse = json_body.unwrap();
    Ok(Json(
        parsed_body
            .data
            .user
            .contributions_collection
            .contribution_calendar
            .weeks
            .iter()
            .flat_map(|week| week.contribution_days.clone())
            .collect(),
    ))
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubContributionsResponse {
    pub data: Data,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub user: User,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub contributions_collection: ContributionsCollection,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionsCollection {
    pub contribution_calendar: ContributionCalendar,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionCalendar {
    pub total_contributions: i64,
    pub weeks: Vec<Week>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Week {
    pub contribution_days: Vec<ContributionDay>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDay {
    pub contribution_count: i64,
    pub date: String,
}
