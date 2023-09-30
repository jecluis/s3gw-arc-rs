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

use std::collections::BTreeMap;

use colored::Colorize;

use crate::{boomln, errorln, version::Version, ws::workspace::Workspace};

#[derive(serde::Deserialize)]
struct GitHubRunResult {
    #[allow(dead_code)]
    total_count: u64,
    workflow_runs: Vec<GitHubWorkflowResult>,
}

#[derive(serde::Deserialize)]
pub struct GitHubWorkflowResult {
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
    let github_config = match &ws.repos.s3gw.config.github {
        Some(c) => c,
        None => {
            boomln!("Expected github repository config, found none!");
            panic!();
        }
    };
    let github_token = &ws.config.user.github_token;

    for relver in releases.values() {
        let tag = format!(
            "{}{}",
            relver.to_str_fmt(&ws.repos.s3gw.config.tag_format),
            match relver.rc {
                None => "".into(),
                Some(v) => format!("-rc{}", v),
            }
        );
        let runs = match github_get_workflows_status(
            &github_config.org,
            &github_config.repo,
            &github_token,
            &tag,
        )
        .await
        {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to obtain workflows for tag '{}'", tag);
                return;
            }
        };

        for run in &runs {
            show_run_status(&tag, &run);
        }
    }
}

pub async fn github_get_workflows_status(
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

fn show_run_status(tag: &String, run: &GitHubWorkflowResult) {
    if run.name.is_none() {
        return;
    }
    let name = run.name.as_ref().unwrap();
    if name.to_lowercase() != "release s3gw" {
        return;
    }

    let status = match &run.status {
        Some(v) => v.clone(),
        None => "unknown".into(),
    };
    let conclusion = match &run.conclusion {
        Some(v) => v.clone(),
        None => "-".into(),
    };

    fn get_status_str(value: &String) -> String {
        match value.as_str() {
            "completed" => "completed".green(),
            "cancelled" => "cancelled".red(),
            "failure" => "failed".red(),
            "success" => "success".green(),
            "in_progress" => "in-progress".yellow(),
            _ => value.bold(),
        }
        .to_string()
    }

    let status_str = get_status_str(&status);
    let conclusion_str = get_status_str(&conclusion);

    let time_delta = if status == "in_progress" {
        chrono::Utc::now() - run.run_started_at
    } else if status == "queued" {
        chrono::Utc::now() - run.created_at
    } else {
        run.updated_at - run.run_started_at
    };

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

    println!(
        "{:20}   status: {}, conclusion: {}  ({})",
        tag, status_str, conclusion_str, duration_str
    );
}
