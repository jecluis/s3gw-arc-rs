// Copyright 2023 SUSE LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{collections::BTreeMap, fmt::Display};

use colored::Colorize;

use crate::{boomln, errorln, version::Version, ws::workspace::Workspace};

#[derive(serde::Deserialize)]
struct GitHubRunResult {
    #[allow(dead_code)]
    total_count: u64,
    workflow_runs: Vec<GitHubWorkflowResult>,
}

#[derive(serde::Deserialize)]
pub(crate) struct GitHubWorkflowResult {
    name: Option<String>,

    #[allow(dead_code)]
    head_branch: Option<String>,
    #[allow(dead_code)]
    head_sha: String,

    status: Option<String>,
    conclusion: Option<String>,

    #[allow(dead_code)]
    display_title: String,
    #[allow(dead_code)]
    created_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    updated_at: chrono::DateTime<chrono::Utc>,
    run_started_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    run_attempt: u64,
    #[allow(dead_code)]
    url: String,
}

pub enum ReleaseWorkflowStatus {
    UNKNOWN,
    QUEUED,
    INPROGRESS,
    COMPLETED,
}

impl Display for ReleaseWorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl ReleaseWorkflowStatus {
    fn to_string(self: &Self) -> String {
        String::from(match &self {
            ReleaseWorkflowStatus::COMPLETED => "completed",
            ReleaseWorkflowStatus::INPROGRESS => "in-progress",
            ReleaseWorkflowStatus::QUEUED => "queued",
            ReleaseWorkflowStatus::UNKNOWN => "unknown",
        })
    }
}

pub struct ReleaseWorkflowResult {
    pub tag: String,
    pub status: ReleaseWorkflowStatus,
    pub success: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl ReleaseWorkflowResult {
    pub fn duration(self: &Self) -> chrono::Duration {
        match &self.status {
            ReleaseWorkflowStatus::UNKNOWN => chrono::Duration::zero(),
            ReleaseWorkflowStatus::INPROGRESS => chrono::Utc::now() - self.started_at,
            ReleaseWorkflowStatus::COMPLETED => self.updated_at - self.created_at,
            ReleaseWorkflowStatus::QUEUED => chrono::Utc::now() - self.created_at,
        }
    }

    pub fn to_duration_str(self: &Self) -> String {
        let time_delta = self.duration();
        let num_days = time_delta.num_days();
        let num_hours = time_delta.num_hours() - (24 * time_delta.num_days());
        let num_minutes = time_delta.num_minutes() - (60 * time_delta.num_hours());
        let num_seconds = time_delta.num_seconds() - (60 * time_delta.num_minutes());

        let mut duration_str = String::new();
        if num_days > 0 {
            duration_str.push_str(format!("{}d", num_days).as_str());
        }
        if num_hours > 0 {
            if duration_str.len() > 0 {
                duration_str.push(' ');
            }
            duration_str.push_str(format!("{}h", num_hours).as_str());
        }
        if num_minutes > 0 {
            if duration_str.len() > 0 {
                duration_str.push(' ');
            }
            duration_str.push_str(format!("{}m", num_minutes).as_str());
        }
        if num_seconds > 0 {
            if duration_str.len() > 0 {
                duration_str.push(' ');
            }
            duration_str.push_str(format!("{}s", num_seconds).as_str());
        }

        duration_str
    }

    pub(crate) fn from_github_result(res: &GitHubWorkflowResult) -> ReleaseWorkflowResult {
        let status = match &res.status {
            None => ReleaseWorkflowStatus::UNKNOWN,
            Some(v) => match v.as_str() {
                "queued" => ReleaseWorkflowStatus::QUEUED,
                "completed" => ReleaseWorkflowStatus::COMPLETED,
                "in_progress" => ReleaseWorkflowStatus::INPROGRESS,
                _ => ReleaseWorkflowStatus::UNKNOWN,
            },
        };

        let success = match &res.conclusion {
            None => false,
            Some(v) => match v.as_str() {
                "success" => true,
                _ => false,
            },
        };

        let tag = match &res.head_branch {
            Some(v) => v.clone(),
            None => {
                panic!("Expected head branch name on workflow result!");
            }
        };

        ReleaseWorkflowResult {
            tag,
            status,
            success,
            created_at: res.created_at,
            updated_at: res.updated_at,
            started_at: res.run_started_at,
        }
    }

    pub fn is_waiting(self: &Self) -> bool {
        match &self.status {
            ReleaseWorkflowStatus::INPROGRESS | ReleaseWorkflowStatus::QUEUED => true,
            _ => false,
        }
    }

    pub fn is_failed(self: &Self) -> bool {
        if self.is_waiting() {
            return false;
        }
        self.success
    }
}

pub async fn status(ws: &Workspace, releases: &BTreeMap<u64, Version>) {
    let is_github_repo = match ws.repos.s3gw.config.github {
        Some(_) => true,
        None => false,
    };
    // github token must be something more than just 'ghp_'
    let has_github_token = ws.config.user.github_token.len() > 4;

    if is_github_repo && has_github_token {
        github_status(&ws, &releases).await;
    } else {
        basic_status(&releases);
    }
}

fn basic_status(releases: &BTreeMap<u64, Version>) {
    for relver in releases.values() {
        println!("- found {}", relver);
    }
}

async fn github_status(ws: &Workspace, releases: &BTreeMap<u64, Version>) {
    for relver in releases.values() {
        let latest_run = match get_release_status(&ws, &relver).await {
            Ok(v) => v,
            Err(()) => {
                boomln!("Unable to obtain latest workflow for version {}", relver);
                return;
            }
        };

        if latest_run.is_some() {
            let run = latest_run.unwrap();
            show_run_status(&run.tag, &run);
        }
    }
}

async fn github_get_workflows_status(
    org: &String,
    repo: &String,
    token: &String,
    tag: &String,
) -> Result<Vec<GitHubWorkflowResult>, ()> {
    let api_url = format!("https://api.github.com/repos/{}/{}/actions/runs", org, repo);

    let response = match reqwest::Client::new()
        .get(&api_url)
        .bearer_auth(&token)
        .query(&[("branch", tag)])
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "s3gw-arc-rs")
        .send()
        .await
    {
        Ok(r) => r,
        Err(err) => {
            errorln!("Unable to obtain github workflows for {}: {}", tag, err);
            return Err(());
        }
    };

    let runs = match response.json::<GitHubRunResult>().await {
        Ok(r) => r.workflow_runs,
        Err(err) => {
            boomln!("Unable to obtain resulting runs: {}", err);
            return Err(());
        }
    };
    return Ok(runs);
}

pub async fn github_get_latest_release_workflow(
    org: &String,
    repo: &String,
    token: &String,
    tag: &String,
) -> Result<Option<ReleaseWorkflowResult>, ()> {
    let mut results = match github_get_workflows_status(&org, &repo, &token, &tag).await {
        Ok(r) => r,
        Err(()) => {
            boomln!("Error obtaining workflow status from github!");
            return Err(());
        }
    };

    if results.is_empty() {
        return Ok(None);
    }

    results.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let filtered: Vec<&GitHubWorkflowResult> = results
        .iter()
        .filter(|e| {
            if e.name.is_none() {
                return false;
            }
            let name = e.name.as_ref().unwrap();
            if name.to_lowercase() != "release s3gw" {
                return false;
            }

            if e.conclusion.is_none() {
                return true;
            } else {
                let c = e.conclusion.as_ref().unwrap();
                return c != "cancelled";
            }
        })
        .collect();

    match filtered.first() {
        None => Ok(None),
        Some(v) => Ok(Some(ReleaseWorkflowResult::from_github_result(&v))),
    }
}

pub async fn get_release_status(
    ws: &Workspace,
    relver: &Version,
) -> Result<Option<ReleaseWorkflowResult>, ()> {
    let github_config = match &ws.repos.s3gw.config.github {
        Some(c) => c,
        None => {
            errorln!("Expected github repository config, found none!");
            return Err(());
        }
    };
    let github_token = &ws.config.user.github_token;
    let tag = format!(
        "{}{}",
        relver.to_str_fmt(&ws.repos.s3gw.config.tag_format),
        match relver.rc {
            None => "".into(),
            Some(v) => format!("-rc{}", v),
        }
    );

    github_get_latest_release_workflow(&github_config.org, &github_config.repo, &github_token, &tag)
        .await
}

fn show_run_status(tag: &String, run: &ReleaseWorkflowResult) {
    let status_str = &run.status.to_string();
    let status = match &run.status {
        ReleaseWorkflowStatus::COMPLETED => status_str.green(),
        ReleaseWorkflowStatus::INPROGRESS => status_str.yellow(),
        ReleaseWorkflowStatus::QUEUED => status_str.bold(),
        ReleaseWorkflowStatus::UNKNOWN => status_str.red(),
    };
    let success = match &run.success {
        true => "success".green(),
        false => match &run.status {
            ReleaseWorkflowStatus::QUEUED | ReleaseWorkflowStatus::INPROGRESS => "waiting".yellow(),
            ReleaseWorkflowStatus::COMPLETED => "failure".red(),
            ReleaseWorkflowStatus::UNKNOWN => "unknown".red(),
        },
    };

    println!(
        "{:20}   status: {}, conclusion: {}  ({})",
        tag,
        status,
        success,
        run.to_duration_str()
    );
}
