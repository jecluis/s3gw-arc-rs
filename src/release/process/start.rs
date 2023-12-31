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

use std::path::PathBuf;

use crate::release::common::{get_release_versions, get_release_versions_from_repo};
use crate::release::errors::ReleaseResult;
use crate::release::process::submodules::{get_submodules, update_submodules};
use crate::version::Version;
use crate::ws::workspace::Workspace;
use crate::{
    boomln, errorln, infoln,
    release::{errors::ReleaseError, ReleaseState},
    successln, warnln,
    ws::repository::Repository,
};

use crate::release::Release;

pub fn start(release: &mut Release, version: &Version, notes: &PathBuf) -> ReleaseResult<()> {
    // 1. sync rw repos to force authorized connect
    // 2. check all repos for existing versions
    // 2.1. make sure this version has not been started in any of the
    //      existing repositories.
    // 3. start release procedures.

    let ws = &release.ws;
    infoln!("Refresh workspace...");
    match ws.sync() {
        Ok(()) => {}
        Err(()) => {
            log::error!("Unable to synchronize workspace repositories!");
            return Err(ReleaseError::SyncError);
        }
    };

    let avail = get_release_versions(&ws, &version);

    if avail.iter().any(|(_, ver)| ver == version) {
        warnln!("Version {} has already been released.", version);
        return Err(ReleaseError::ReleaseExistsError);
    }

    if avail.len() > 0 {
        warnln!("Release version {} has already been started.", version);
        return Err(ReleaseError::ReleaseStartedError);
    }

    // Check whether the release has been started across the various
    // repositories. If it has been started in any one repository, yet not
    // started in the 's3gw' repository (otherwise it would have been caught
    // above), then we have a potentially corrupted release state.

    let mut started_repos: Vec<String> = vec![];
    for repo in ws.repos.as_vec() {
        let versions = get_release_versions_from_repo(&repo, &version);
        if versions.len() > 0 {
            started_repos.push(repo.name.clone());
        }
    }
    if started_repos.len() > 0 {
        warnln!(
            "Release version {} has been started in some repositories: {}",
            version,
            started_repos.join(", ")
        );
        errorln!("Release potentially corrupted!");
        return Err(ReleaseError::CorruptedError);
    }

    infoln!("Start releasing version {}", version);

    match create_release_branches(&ws, &version) {
        Ok(true) => {
            successln!("Created release branches.");
        }
        Ok(false) => {
            infoln!("Release branches already exist.");
        }
        Err(err) => {
            errorln!("Error creating release branches: {}", err);
            return Err(err);
        }
    };

    // write down release version state to disk -- makes sure this workspace
    // is bound to this release until it is finished (or the file is
    // removed).
    release.state = Some(ReleaseState {
        release_version: version.clone(),
    });
    match release.write() {
        Ok(()) => {}
        Err(()) => {
            boomln!("Unable to write release state file!");
            return Err(ReleaseError::UnknownError);
        }
    };

    match crate::release::sync::sync(&release, &version) {
        Ok(()) => {
            infoln!("Synchronized release repositories");
        }
        Err(()) => {
            errorln!("Unable to synchronize release repositories!");
            return Err(ReleaseError::SyncError);
        }
    };

    // start a new release version release candidate.
    match start_release_candidate(&ws, &version, Some(&notes)) {
        Ok(ver) => {
            if let Some(rc) = ver.rc {
                if rc != 1 {
                    // somehow this is not an "-rc1", which is unexpected
                    // given we are just starting a new release. Consider
                    // release corrupted!
                    boomln!("Release is corrupted. Expected '-rc1', got '-rc{}'!", rc);
                    return Err(ReleaseError::CorruptedError);
                }
            } else {
                // expected an RC and didn't get one! Something is wrong!
                errorln!("Started release is not a release candidate. Got '{}'.", ver);
                return Err(ReleaseError::CorruptedError);
            }
        }
        Err(err) => {
            errorln!("Unable to start v{}-rc1: {}", version, err);
            return Err(err);
        }
    };

    Ok(())
}

/// Prepare release branches by creating them if necessary.
///
fn create_release_branches(ws: &Workspace, version: &Version) -> ReleaseResult<bool> {
    let mut res = false;
    // check whether we need to cut branches for each repository
    match maybe_cut_branches(&ws, &version) {
        Ok(None) => {
            log::info!("Branches ready to start release!");
        }
        Ok(Some(repos)) => {
            match cut_branches_for(&version, &repos) {
                Ok(()) => {
                    log::info!("Success cutting branches for v{}", version);
                    res = true;
                }
                Err(err) => {
                    log::error!("Error cutting branches for v{}", version);
                    return Err(err);
                }
            };
        }
        Err(err) => {
            log::error!("Unable to cut branches for release {}: {}", version, err);
            return Err(err);
        }
    };

    Ok(res)
}

/// Check whether we need to cut release branches, and, if so, for which repositories.
///
fn maybe_cut_branches<'a>(
    ws: &'a Workspace,
    version: &Version,
) -> ReleaseResult<Option<Vec<&'a Repository>>> {
    let repos = ws.repos.as_vec();
    let base_version = version.get_base_version();
    let base_version_id = base_version.get_version_id();

    let mut repos_to_cut: Vec<&Repository> = vec![];
    for repo in &repos {
        let branches = match repo.get_release_branches() {
            Ok(v) => v,
            Err(err) => {
                log::error!("unable to obtain branches for release: {}", err);
                return Err(ReleaseError::UnknownError);
            }
        };
        for (k, v) in &branches {
            log::debug!("Found branch '{}' ({})", v, k);
        }
        if !branches.contains_key(&base_version_id) {
            repos_to_cut.push(repo);
        }
    }

    if repos_to_cut.len() == 0 {
        return Ok(None);
    } else if repos_to_cut.len() != repos.len() {
        return Err(ReleaseError::CorruptedError);
    }

    infoln!(
        "Need to cut release branches for v{} on repositories {}",
        base_version,
        repos_to_cut
            .iter()
            .map(|x: &&Repository| x.name.clone())
            .collect::<Vec<String>>()
            .join(", ")
    );
    match inquire::Confirm::new("Cut required branches?")
        .with_default(true)
        .prompt()
    {
        Ok(true) => {}
        Ok(false) => {
            println!("abort release");
            return Err(ReleaseError::AbortedError);
        }
        Err(e) => {
            log::error!("Error prompting user: {}", e);
            return Err(ReleaseError::UnknownError);
        }
    };

    Ok(Some(repos_to_cut))
}

/// Cut release branches for the provided repositories, for the provided
/// release version.
///
fn cut_branches_for(version: &Version, repos: &Vec<&Repository>) -> ReleaseResult<()> {
    for repo in repos {
        log::info!("cut branch for repository {}", repo.name);
        match repo.branch_version_from_default(&version) {
            Ok(()) => {
                log::info!("branched off!");
            }
            Err(err) => {
                log::error!("error branching off: {}", err);
                return Err(ReleaseError::UnknownError);
            }
        }
    }

    Ok(())
}

/// Start a new release candidate. If 'notes' is provided, then we will move
/// the provided file to the 's3gw' repo's release notes file before
/// finalizing the release candidate.
///
pub fn start_release_candidate(
    ws: &Workspace,
    relver: &Version,
    notes: Option<&PathBuf>,
) -> ReleaseResult<Version> {
    // figure out which rc comes next.
    infoln!("Assess next release version...");
    let avail_versions = get_release_versions(&ws, &relver);
    let next_rc = match avail_versions.last_key_value() {
        None => 1_u64,
        Some((_, v)) => {
            if let Some(rc) = v.rc {
                rc + 1
            } else {
                log::error!("Highest version is not an RC. Maybe release? Found: {}", v);
                return Err(ReleaseError::UnknownError);
            }
        }
    };

    let mut next_ver = relver.clone();
    next_ver.rc = Some(next_rc);

    infoln!("Start next release candidate '{}': {}", next_rc, next_ver);

    match perform_release(&ws, &relver, &next_ver, &notes) {
        Ok(()) => {
            successln!("Started release ver '{}' tag '{}'", relver, next_ver);
            Ok(next_ver)
        }
        Err(err) => {
            errorln!("Error performing release {}: {}", next_ver, err);
            Err(err)
        }
    }
}

/// Perform a release, by creating appropriate tags and ensuring the 's3gw' repo
/// represents the correct state for said release.
/// This is used to start a new release candidate, as well to finish a release.
///
pub fn perform_release(
    ws: &Workspace,
    relver: &Version,
    next_ver: &Version,
    notes: &Option<&PathBuf>,
) -> ReleaseResult<()> {
    // start release candidate on the various repositories, except
    // 's3gw.git'.
    let mut submodules = get_submodules(&ws);

    infoln!("Tagging repositories...");
    for entry in &mut submodules {
        log::debug!(
            "Tagging repository '{}' with version '{}'",
            entry.repo.name,
            next_ver
        );
        match entry.repo.tag_release_branch(&relver, &next_ver) {
            Ok((tag_name, tag_oid)) => {
                log::debug!(
                    "Tagged version '{}' with '{}' oid {} name {}",
                    relver,
                    next_ver,
                    tag_oid,
                    tag_name,
                );
            }
            Err(err) => {
                errorln!(
                    "Error tagging version '{}' with '{}': {}",
                    relver,
                    next_ver,
                    err
                );
                return Err(ReleaseError::TaggingError);
            }
        };
    }

    // repositories have been tagged -- push them out so we can update the
    // submodules on 's3gw.git'.
    infoln!("Pushing repositories...");
    for entry in &submodules {
        log::debug!("Pushing '{}' to repository '{}'", relver, entry.name);
        match entry.repo.push_release_branch(&relver) {
            Ok(()) => {
                log::debug!("Pushed '{}' to repository '{}'", relver, entry.name);
            }
            Err(err) => {
                errorln!(
                    "Error pushing '{}' to repository '{}': {}",
                    relver,
                    entry.name,
                    err
                );
                return Err(ReleaseError::PushingError);
            }
        };

        match entry.repo.push_release_tag(&next_ver) {
            Ok(()) => {
                log::debug!("Pushed '{}' to repository '{}'!", next_ver, entry.name);
            }
            Err(err) => {
                errorln!(
                    "Error pushing '{}' to repository '{}': {}",
                    next_ver,
                    entry.name,
                    err
                );
                return Err(ReleaseError::PushingError);
            }
        };
    }

    let mut paths_to_add: Vec<PathBuf> = vec![];

    // update submodules on 's3gw.git' to reflect the current state of each
    // repository.
    infoln!("Updating submodules...");
    let mut sub_paths = match update_submodules(&ws, &next_ver) {
        Ok(v) => {
            infoln!("Updated submodules to {}", next_ver);
            v
        }
        Err(()) => {
            errorln!("Error updating submodules to {}", next_ver);
            return Err(ReleaseError::SubmoduleError);
        }
    };
    paths_to_add.append(&mut sub_paths);

    infoln!("Finalizing release...");
    if let Some(notes_file) = notes {
        // copy release notes file to its final destination.
        let release_notes_dir = PathBuf::from("docs/release-notes");
        let release_notes_file =
            PathBuf::from(format!("s3gw-v{}.md", next_ver.get_release_version()));
        let release_notes_path = release_notes_dir.join(&release_notes_file);
        let release_notes_path_abs = ws.repos.s3gw.path.join(&release_notes_path);
        let latest_path = release_notes_dir.join(PathBuf::from("latest"));
        let latest_path_abs = ws.repos.s3gw.path.join(&latest_path);

        match std::fs::copy(&notes_file, &release_notes_path_abs) {
            Ok(_) => {}
            Err(err) => {
                boomln!(
                    "Error copying notes file from '{}' to '{}': {}",
                    notes_file.display(),
                    release_notes_path_abs.display(),
                    err
                );
                return Err(ReleaseError::UnknownError);
            }
        };
        if latest_path_abs.is_symlink() {
            std::fs::remove_file(&latest_path_abs).expect("Unable to remove 'latest' symlink!");
        }
        match std::os::unix::fs::symlink(&release_notes_file, &latest_path_abs) {
            Ok(_) => {}
            Err(err) => {
                boomln!("Error updating 'latest' symlink: {}", err);
                return Err(ReleaseError::UnknownError);
            }
        };
        paths_to_add.push(release_notes_path);
        paths_to_add.push(latest_path);
    }

    let mut force_empty_commit = false;

    if paths_to_add.len() > 0 {
        match ws.repos.s3gw.stage_paths(&paths_to_add) {
            Ok(()) => {
                log::debug!(
                    "Staged paths:\n{}",
                    paths_to_add
                        .iter()
                        .map(|e| e.display().to_string())
                        .collect::<Vec<String>>()
                        .join("\n")
                );
            }
            Err(err) => {
                log::error!("Error staging paths: {}", err);
                return Err(ReleaseError::StagingError);
            }
        };
    } else {
        warnln!("No changes on repositories, continuing anyway.");
        force_empty_commit = true;
    }

    match ws
        .repos
        .s3gw
        .commit_release(&relver, &next_ver, force_empty_commit)
    {
        Ok(()) => {
            log::debug!("Committed release '{}' tag '{}'", relver, next_ver);
        }
        Err(err) => {
            errorln!(
                "Unable to commit release '{}' tag '{}': {}",
                relver,
                next_ver,
                err
            );
            return Err(ReleaseError::CommittingError);
        }
    };

    // finally, push the branch and the release tag.
    match ws.repos.s3gw.push_release_branch(&relver) {
        Ok(()) => {
            log::debug!("Pushed s3gw release branch for '{}'", relver);
        }
        Err(err) => {
            errorln!(
                "Error pushing s3gw release branch for '{}': {}",
                relver,
                err
            );
            return Err(ReleaseError::PushingError);
        }
    };

    match ws.repos.s3gw.push_release_tag(&next_ver) {
        Ok(()) => {
            log::debug!(
                "Pushed s3gw release tag '{}' for version '{}'",
                next_ver,
                relver
            );
        }
        Err(err) => {
            errorln!(
                "Error pushing s3gw release tag '{}' for version '{}': {}",
                next_ver,
                relver,
                err
            );
            return Err(ReleaseError::PushingError);
        }
    };

    Ok(())
}
