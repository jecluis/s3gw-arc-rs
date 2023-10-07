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

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use colored::Colorize;

use crate::{
    boomln,
    common::UpdateProgress,
    errorln,
    version::Version,
    ws::{repository::Repository, workspace::Workspace},
};

use super::common;

// ----
// raw responses from GitHub for workflow runs
// ----

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
    run_attempt: u64,
    #[allow(dead_code)]
    url: String,
}

/// ----
/// end of raw responses from GitHub for workflow runs
/// ----

/// ----
/// raw responses from Quay.io for repository tags
/// ----

#[derive(serde::Deserialize)]
pub(crate) struct QuayRepositoryTagResult {
    tags: HashMap<String, QuayRepositoryTagEntry>,
}

#[derive(serde::Deserialize)]
pub(crate) struct QuayRepositoryTagEntry {
    #[allow(dead_code)]
    name: String,
}

/// ----
/// end of raw responses from Quay.io for repository tags
/// ----

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
    pub num_attempts: u64,
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
            num_attempts: res.run_attempt,
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

pub(crate) struct QuayStatus {
    s3gw: HashMap<String, QuayRepositoryTagEntry>,
    ui: HashMap<String, QuayRepositoryTagEntry>,
}

/// Print release status for each release version in the provided 'releases'
/// tree. This function will obtain information for each release from multiple
/// sources, including the local repositories, github, and quay.
///
pub async fn status(ws: &Workspace, version: &Version, releases: &BTreeMap<u64, Version>) {
    let progress = UpdateProgress::new(&"gather information".into());
    progress.start();

    let is_github_repo = match ws.repos.s3gw.config.github {
        Some(_) => true,
        None => false,
    };
    // github token must be something more than just 'ghp_'
    let has_github_token = ws.config.user.github_token.len() > 4;

    let quay_status = match get_quay_status(&ws).await {
        Ok(res) => res,
        Err(()) => None,
    };

    let mut table = crate::release::common::StatusTable::default();
    for relver in releases.values() {
        let table_entry = table.new_entry(&relver);

        let diff_str = get_commit_diff_status_str(&ws.repos.s3gw, &relver);
        table_entry.add_record(&diff_str);

        // get github status
        if is_github_repo && has_github_token {
            if let Some(s) = get_github_status_str(&ws, &relver).await {
                table_entry.add_record(&s);
            }
        }
        // get image tag status from quay
        if let Some(s) = &quay_status {
            let status_str = get_quay_status_str(&relver, &s);
            table_entry.add_record(&status_str);
        }
    }

    progress.finish();
    println!("{}", table);

    match show_per_repo_diff(&ws, &version) {
        Ok(()) => {}
        Err(()) => {
            boomln!("Error obtaining per repository commit diffs");
            return;
        }
    };
}

/// Returns a prettified release workflow run status string for the specified
/// release version, if any is available.
///
async fn get_github_status_str(ws: &Workspace, relver: &Version) -> Option<String> {
    let latest_run = match get_release_status(&ws, &relver).await {
        Ok(v) => v,
        Err(()) => {
            errorln!("Unable to obtain latest workflow for version {}", relver);
            return None;
        }
    };
    if latest_run.is_some() {
        return Some(get_github_run_status_str(&latest_run.unwrap()));
    }
    None
}

/// Obtain workflow runs from specified 'org' and 'repo', for the specified
/// tag/branch 'tag'. Returns a vector of 'GitHubWorkflowResult', containing the
/// raw response from github for each individual workflow run matching said
/// 'tag'. Result needs to be handled by the caller to make it useful.
///
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

/// Obtain the latest release workflow for the specified tag or branch, 'tag',
/// for the specified 'org' and 'repo'. This function works by first obtaining
/// all workflow runs that match said 'tag', and then filtering them for the
/// specific release workflow name, ignoring those that are yet to be populated
/// (this should not happen by the way), and finally returning solely the latest
/// one, if available.
///
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

/// Obtain release status from github, for the specified release version. This
/// function is simply a helper to translate our github configuration into
/// something that can be called against github. Returns the latest workflow run
/// available for the provided release version, if any is available.
///
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

/// Returns a status string for a given release workflow run, with pretty formatting.
///
fn get_github_run_status_str(run: &ReleaseWorkflowResult) -> String {
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

    format!(
        "build status: {}, conclusion: {}  {:12}  ({} attempt{})",
        status,
        success,
        run.to_duration_str(),
        run.num_attempts,
        if run.num_attempts == 1 { "" } else { "s" }
    )
}

/// Obtain all tags from the specified repository 'repo' in the namespace 'ns',
/// from quay.io. Returns a hash map of 'QuayRepositoryTagEntry', if
/// successfull.
///
async fn quay_get_tags(repo: &String) -> Result<HashMap<String, QuayRepositoryTagEntry>, ()> {
    let api_url = format!("https://quay.io/api/v1/repository/{}", repo);

    let response = match reqwest::Client::new()
        .get(&api_url)
        .query(&[("includeTags", "true")])
        .send()
        .await
    {
        Ok(r) => r,
        Err(err) => {
            errorln!("Unable to obtain tags from quay for '{}': {}", repo, err);
            return Err(());
        }
    };

    let tags = match response.json::<QuayRepositoryTagResult>().await {
        Ok(r) => r.tags,
        Err(err) => {
            boomln!(
                "Unable to obtain resulting tags from quay for '{}': {}",
                repo,
                err
            );
            return Err(());
        }
    };
    Ok(tags)
}

/// Obtain status from quay for the various repositories we want.
///
async fn get_quay_status(ws: &Workspace) -> Result<Option<QuayStatus>, ()> {
    let cfg = match &ws.config.registry {
        Some(c) => c,
        None => return Ok(None),
    };

    let s3gw = if let Ok(res) = quay_get_tags(&cfg.s3gw).await {
        res
    } else {
        return Err(());
    };
    let ui = if let Ok(res) = quay_get_tags(&cfg.ui).await {
        res
    } else {
        return Err(());
    };

    Ok(Some(QuayStatus { s3gw, ui }))
}

/// Obtain status string from quay for a specific release version.
///
fn get_quay_status_str(relver: &Version, quay_status: &QuayStatus) -> String {
    let relstr = format!("v{}", relver);

    fn get_status_from_map(
        map: &HashMap<String, QuayRepositoryTagEntry>,
        relstr: &String,
    ) -> String {
        if let Some(_) = map.get(relstr) {
            "found".green().to_string()
        } else {
            "not found".yellow().to_string()
        }
    }

    let s3gw_str = get_status_from_map(&quay_status.s3gw, &relstr);
    let ui_str = get_status_from_map(&quay_status.ui, &relstr);
    format!("images: s3gw = {}, s3gw-ui = {}", s3gw_str, ui_str)
}

/// Obtain a human readable string stating the commit difference for the
/// specified 'target'.
///
fn get_human_readable_diff(
    ahead: usize,
    behind: usize,
    source: Option<&String>,
    target: &String,
) -> String {
    fn do_plural(value: usize) -> String {
        if value > 1 {
            "s".into()
        } else {
            "".into()
        }
    }

    let source_str = if source.is_some() {
        format!("{} is ", source.unwrap())
    } else {
        "".into()
    };

    if ahead == 0 && behind == 0 {
        return format!("{}up to date with {}", source_str, target);
    }

    format!(
        "{}{}{}{} {}",
        source_str,
        if ahead > 0 {
            format!("{} commit{} ahead", ahead, do_plural(ahead))
        } else {
            "".into()
        },
        if ahead > 0 && behind > 0 { "," } else { "" },
        if behind > 0 {
            format!("{} commit{} behind", behind, do_plural(behind))
        } else {
            "".into()
        },
        target,
    )
}

/// Obtain status string representing commit distance from 'relver' to its
/// release branch's HEAD.
///
fn get_commit_diff_status_str(repo: &Repository, relver: &Version) -> String {
    let (ahead, behind) = repo.diff_head(&relver, true).unwrap();
    get_human_readable_diff(ahead, behind, None, &"HEAD".into())
}

/// Print per repository commit difference status, between latest available
/// release for the provided version 'relver' and the HEAD of the release branch.
///
fn show_per_repo_diff(ws: &Workspace, relver: &Version) -> Result<(), ()> {
    let repos = ws.repos.as_vec();

    for repo in repos {
        match show_repo_diff(&repo, &relver) {
            Ok(()) => {}
            Err(()) => {
                errorln!("Unable to get repository '{}' commit diff", repo.name);
            }
        };
    }
    println!("");

    Ok(())
}

/// Obtain the commit difference between a repository's release branch HEAD and
/// its latest release tag.
///
fn show_repo_diff(repo: &Repository, relver: &Version) -> Result<(), ()> {
    let releases = common::get_release_versions_from_repo(&repo, relver);
    let latest_release = match releases.keys().max() {
        Some(v) => releases.get(v).unwrap(),
        None => {
            errorln!(
                "Release '{}' not found for repository '{}': possibly corrupted release!",
                relver,
                repo.name
            );
            return Err(());
        }
    };

    let (ahead, behind) = match repo.diff_head(&latest_release, true) {
        Ok(res) => res,
        Err(err) => {
            errorln!(
                "Error obtaining commit diff for release '{}' in repository '{}': {}",
                latest_release,
                repo.name,
                err
            );
            return Err(());
        }
    };

    let release_str = latest_release.to_string();
    let branch_str = latest_release.get_base_version_str();

    let diff_str = get_human_readable_diff(ahead, behind, Some(&release_str), &branch_str);
    println!("{:12}: {}", repo.name, diff_str);
    Ok(())
}
