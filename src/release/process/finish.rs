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

use std::{io::Write, path::PathBuf};

use crate::{
    boomln, errorln, infoln,
    release::sync,
    release::{
        errors::ReleaseResult,
        process::{charts, start},
    },
    successln,
    version::Version,
    ws::{repository::Repository, workspace::Workspace},
};

use crate::release::{errors::ReleaseError, Release};

#[derive(serde::Serialize)]
struct CreatePullRequestRequest {
    title: String,
    head: String,
    base: String,
    body: String,
}

#[derive(serde::Deserialize)]
struct CreatePullRequestResponse {
    pub html_url: String,
    pub number: i64,
}

pub async fn finish(release: &mut Release, version: &Version) -> ReleaseResult<()> {
    // 1. check whether release has been finished
    // 2. check whether release has been started
    // 3. sync repositories for the specified release
    // 4. find the highest release candidate
    // 5. adjust charts version
    // 6. perform the release, via start::perform_release()
    // 7. push out final release.

    let ws = &release.ws;

    let release_versions = crate::release::common::get_release_versions(&ws, &version);
    if release_versions.contains_key(&version.get_version_id()) {
        errorln!("Release version {} already exists", version);
        return Err(ReleaseError::ReleaseExistsError);
    } else if release_versions.len() == 0 {
        errorln!("Release has not been started yet.");
        return Err(ReleaseError::NotStartedError);
    }

    infoln!("Continuing release {}", version);

    match sync::sync(&release, &version) {
        Ok(()) => {}
        Err(()) => {
            errorln!("Unable to sync release!");
            return Err(ReleaseError::SyncError);
        }
    };

    let max = match release_versions.last_key_value() {
        None => {
            errorln!("Could not find the highest release candidate!");
            return Err(ReleaseError::CorruptedError);
        }
        Some((_, v)) => v,
    };
    infoln!("Basing release on highest candidate: {}", max);

    // adjust charts version

    infoln!("Update chart to version {}", version);
    if let Err(err) = charts::update_charts(&ws.repos.charts, &version) {
        boomln!("Error updating chart: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    match start::perform_release(&ws, &version, &version, &None) {
        Ok(()) => {}
        Err(err) => {
            errorln!("Unable to finish release for {}: {}", version, err);
            return Err(err);
        }
    };

    // push final chart branch
    //  This is a workaround that avoids releasing the chart until we
    //  effectively are ready to finish the release. So far we have been pushing
    //  to a "temporary" branch for the current release, but now we need to have
    //  a specific branch name in the charts repository so the release workflow
    //  can be triggered.

    infoln!("Finalizing Helm Chart release");
    if let Err(err) = charts::finalize_charts_release(&ws.repos.charts, &version) {
        errorln!("Unable to finalize chart for publishing: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    // open pull request against s3gw.git's "main"
    //  This ensures we have a pull request ready with the new release notes, as
    //  well as updated documentation.
    infoln!("Finalizing release");
    if let Err(err) = finish_s3gw_update_default(&ws, &ws.repos.s3gw, &version).await {
        errorln!("Unable to finalize s3gw repository's release: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    successln!("Version {} released!", version);

    Ok(())
}

/// Finish releasing the s3gw.git repository by opening a pull request against
/// its default branch (most likely 'main'), with a patch set including the
/// release's changelog and an update to the 'mkdocs.yml' file with an entry for
/// the new release notes.
///
async fn finish_s3gw_update_default(
    ws: &Workspace,
    repo: &Repository,
    relver: &Version,
) -> ReleaseResult<()> {
    match repo.update(false) {
        Ok(()) => {
            log::trace!("Synchronized repository {} with upstream", repo.name);
        }
        Err(err) => {
            errorln!(
                "Unable to synchronize repository {} with upstream: {}",
                repo.name,
                err
            );
            return Err(ReleaseError::UnknownError);
        }
    }

    // lets get the release notes file first for the release, from the release branch.

    match repo.checkout_version_branch(&relver.get_base_version()) {
        Ok(()) => {
            log::trace!("Checked out version branch for {}", relver);
        }
        Err(err) => {
            errorln!("Unable to checkout version branch for {}: {}", relver, err);
            return Err(ReleaseError::UnknownError);
        }
    };

    let relver_notes_path = PathBuf::from(format!(
        "docs/release-notes/s3gw-v{}.md",
        relver.get_release_version()
    ));
    let relver_notes_path_abs = repo.path.join(&relver_notes_path);
    if !relver_notes_path_abs.exists() {
        log::error!(
            "Unable to find release notes file for {} at '{}'",
            relver,
            relver_notes_path_abs.display()
        );
        log::error!("Potentially corrupted release!");
        return Err(ReleaseError::CorruptedError);
    }

    let tmpfile = match tempfile::NamedTempFile::new() {
        Ok(f) => f,
        Err(err) => {
            log::error!("Unable to create temporary file: {}", err);
            return Err(ReleaseError::UnknownError);
        }
    };
    if let Err(err) = std::fs::copy(&relver_notes_path_abs, &tmpfile.path()) {
        log::error!(
            "Error copying release notes from '{}' to '{}': {}",
            relver_notes_path.display(),
            tmpfile.path().display(),
            err
        );
        return Err(ReleaseError::UnknownError);
    }

    // checkout default branch to a new branch, from which we will open a pull
    // request with the updated notes and docs.
    let branch_suffix = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let dst_branch = format!("release-v{}-{}", relver, branch_suffix);

    if let Err(err) = repo.branch_from_default(&dst_branch) {
        log::error!("Unable to branch default to '{}': {}", dst_branch, err);
        return Err(ReleaseError::UnknownError);
    }

    // copy release notes to this new branch
    if let Err(err) = std::fs::copy(&tmpfile.path(), &relver_notes_path_abs) {
        log::error!(
            "Error copying release notes from '{}' to '{}': {}",
            tmpfile.path().display(),
            relver_notes_path.display(),
            err
        );
        return Err(ReleaseError::UnknownError);
    }

    let mkdocs_path = PathBuf::from("mkdocs.yml");
    let mkdocs_path_abs = repo.path.join(&mkdocs_path);
    if !mkdocs_path_abs.exists() {
        log::error!(
            "Unable to find mkdocs.yml file at {}",
            mkdocs_path_abs.display()
        );
        return Err(ReleaseError::UnknownError);
    }
    if let Err(err) = adjust_mkdocs(&mkdocs_path_abs, &relver) {
        log::error!("Error adjusting mkdocs file: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    // update s3gw submodules to match release
    let mut subpaths = match super::submodules::update_submodules(&ws, &relver) {
        Ok(p) => p,
        Err(()) => {
            log::error!("Error updating submodules!");
            return Err(ReleaseError::UnknownError);
        }
    };

    let mut to_stage = vec![mkdocs_path, relver_notes_path];
    to_stage.append(&mut subpaths);

    // stage paths and commit
    if let Err(err) = repo.stage_paths(&to_stage) {
        log::error!("Error staging paths: {}", err);
        return Err(ReleaseError::StagingError);
    }

    let commit_msg = format!("Release v{}", relver);
    if let Err(err) = repo.commit(&commit_msg) {
        log::error!(
            "Error committing release commit for {} on branch {}: {}",
            relver,
            dst_branch,
            err
        );
        return Err(ReleaseError::UnknownError);
    }

    // push and open pull request
    if let Err(err) = create_pull_request(&ws, &repo, &dst_branch, &relver).await {
        log::error!(
            "Error creating pull request for '{}' on repository '{}': {}",
            dst_branch,
            repo.name,
            err
        );
        return Err(ReleaseError::UnknownError);
    }

    Ok(())
}

/// Adjust the 'mkdocs.yml' file to reflect the latest release.
///
fn adjust_mkdocs(path: &PathBuf, relver: &Version) -> ReleaseResult<()> {
    // the version to add to the mkdocs file
    let relver_str = format!("v{}", relver.get_release_version());
    let relnotes_str = format!("release-notes/s3gw-{}.md", relver_str);

    let f = std::fs::File::open(&path).unwrap();
    let mut data: serde_yaml::Value = match serde_yaml::from_reader(f) {
        Err(err) => {
            println!("Error reading yaml: {}", err);
            return Err(ReleaseError::UnknownError);
        }
        Ok(v) => v,
    };

    for entry in data["nav"].as_sequence_mut().unwrap() {
        if entry.is_mapping() {
            let mapping = entry.as_mapping_mut().unwrap();
            if !mapping.contains_key("Release Notes") {
                continue;
            }

            let rl = mapping.get_mut("Release Notes").unwrap();
            let mut new_mapping = serde_yaml::Mapping::new();
            new_mapping.insert(
                serde_yaml::Value::String(relver_str.clone()),
                serde_yaml::Value::String(relnotes_str.clone()),
            );

            rl.as_sequence_mut()
                .unwrap()
                .push(serde_yaml::Value::Mapping(new_mapping));
        }
    }

    let output = serde_yaml::to_string(&data).unwrap();
    log::trace!("pre output mkdocs: {}", output);

    // run through yaml_rust so we can get properly indented yaml

    let yaml_doc = yaml_rust::YamlLoader::load_from_str(&output).unwrap();
    let mut yaml_out = String::new();
    let mut emitter = yaml_rust::YamlEmitter::new(&mut yaml_out);
    emitter.dump(&yaml_doc[0]).unwrap();

    assert!(!yaml_out.is_empty());

    // remove document separator
    yaml_out.push('\n');
    let res = yaml_out.strip_prefix("---\n").unwrap();
    log::trace!("resulting mkdocs: {}", res);

    let mut outfile = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    outfile.write(res.as_bytes()).unwrap();
    outfile.flush().unwrap();

    Ok(())
}

/// Create a pull request from the specified branch, for the specified release
/// version, on the 's3gw' repository.
///
///  note(joao): We could have assumed the 's3gw' repository, and used that from
///  the 'workspace' provided. However, we may want this function later on for
///  other repositories, like the 'charts' repository.
///
async fn create_pull_request(
    ws: &Workspace,
    repo: &Repository,
    branch: &String,
    relver: &Version,
) -> ReleaseResult<()> {
    // push branch to repository
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
    if let Err(err) = repo.push(&refspec) {
        log::error!("Unable to push '{}' to remote repository: {}", branch, err);
        return Err(ReleaseError::PushingError);
    }

    // open pull request against default branch
    let default_branch = match repo.get_default_branch_name() {
        Ok(v) => v,
        Err(err) => {
            log::error!(
                "Unable to obtain default branch name for repository '{}': {}",
                repo.name,
                err
            );
            return Err(ReleaseError::UnknownError);
        }
    };
    let gh_config = match &repo.config.github {
        None => {
            log::error!("GitHub repository not configured, can't open pull request!");
            return Err(ReleaseError::UnknownError);
        }
        Some(c) => c,
    };
    let user_config = &ws.config.user;
    if user_config.github_token.is_empty() {
        log::error!("GitHub token not configured, can't open pull request!");
        return Err(ReleaseError::UnknownError);
    }

    let api_url = format!(
        "https://api.github.com/repos/{}/{}/pulls",
        gh_config.org, gh_config.repo
    );

    let req = CreatePullRequestRequest {
        title: format!("Release v{}", relver),
        body: format!(
            "Updates '{}' to reflect v{}\n\nSigned-off-by: {} \\<{}>",
            default_branch, relver, user_config.name, user_config.email
        ),
        head: branch.clone(),
        base: default_branch.clone(),
    };

    match serde_json::to_string(&req) {
        Ok(v) => {
            log::trace!("request body:\n{}", v);
        }
        Err(err) => {
            log::error!("Unable to encode request body: {}", err);
            return Err(ReleaseError::UnknownError);
        }
    };

    let response = match reqwest::Client::new()
        .post(&api_url)
        .bearer_auth(&user_config.github_token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "s3gw-arc-rs")
        .json(&req)
        .send()
        .await
    {
        Ok(r) => r,
        Err(err) => {
            log::error!(
                "Unable to open pull request for '{}' against '{}' on '{}/{}': {}",
                branch,
                default_branch,
                gh_config.org,
                gh_config.repo,
                err
            );
            return Err(ReleaseError::UnknownError);
        }
    };

    let res_body = match response.text().await {
        Ok(v) => {
            log::trace!("response body:\n{}", v);
            v
        }
        Err(err) => {
            log::error!("Error obtaining response body: {}", err);
            return Err(ReleaseError::UnknownError);
        }
    };

    let (url, number) = match serde_json::from_str::<CreatePullRequestResponse>(&res_body) {
        Ok(r) => (r.html_url, r.number),
        Err(err) => {
            log::error!("Unable to obtain pull request URL and id: {}", err);
            return Err(ReleaseError::UnknownError);
        }
    };
    successln!("Opened Pull Request {} at {}", number, url);

    Ok(())
}
