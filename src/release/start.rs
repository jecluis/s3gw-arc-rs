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

use crate::{
    boomln, errorln, infoln,
    release::{errors::ReleaseError, ReleaseState},
    successln, warnln,
    ws::{repository::Repository, version::Version},
};

use super::Release;

struct SubmoduleInfo<'a> {
    name: String,
    repo: &'a Repository,
    tag_oid: Option<String>,
    tag_name: Option<String>,
    push_rc_tags: bool,
}

impl Release {
    pub fn start(self: &mut Self, version: &Version, notes: &PathBuf) -> Result<(), ()> {
        // 1. sync rw repos to force authorized connect
        // 2. check all repos for existing versions
        // 2.1. make sure this version has not been started in any of the
        //      existing repositories.
        // 3. start release procedures.

        if let Some(s) = &self.state {
            warnln!("On-going release detected!");
            if &s.release_version == version {
                infoln!("Maybe you want to 'continue' instead?");
            } else {
                infoln!(format!(
                    "Detected version {}, attempting to start {}!",
                    s.release_version, version
                ));
            }
            return Err(());
        }

        infoln!("Refresh workspace...");
        match self.ws.sync() {
            Ok(()) => {}
            Err(()) => {
                log::error!("Unable to synchronize workspace repositories!");
                return Err(());
            }
        };

        let avail = self.get_release_versions(&version);
        let mut avail_it = avail.iter();

        if avail_it.any(|(_, ver)| ver == version) {
            warnln!(format!("Version {} has already been released.", version));
            return Err(());
        }

        // NOTE(joao): we should check whether there is a started release across
        // the repositories. This can be done by checking for rc versions on
        // every repository. For now we will ignore this bit.

        if avail_it.count() > 0 {
            warnln!(format!(
                "Release version {} has already been started.",
                version
            ));
            return Err(());
        }

        infoln!(format!("Start releasing version {}", version));

        match self.create_release_branches(&version) {
            Ok(true) => {
                successln!("Created release branches.");
            }
            Ok(false) => {
                infoln!("Release branches already exist.");
            }
            Err(()) => {
                errorln!("Error creating release!");
                return Err(());
            }
        };

        // write down release version state to disk -- makes sure this workspace
        // is bound to this release until it is finished (or the file is
        // removed).
        self.state = Some(ReleaseState {
            release_version: version.clone(),
        });
        match self.write() {
            Ok(()) => {}
            Err(()) => {
                boomln!("Unable to write release state file!");
                return Err(());
            }
        };

        // start a new release version release candidate.
        match self.start_release_candidate(Some(&notes)) {
            Ok(ver) => {
                if let Some(rc) = ver.rc {
                    if rc != 1 {
                        // somehow this is not an "-rc1", which is unexpected
                        // given we are just starting a new release. Consider
                        // release corrupted!
                        boomln!(format!(
                            "Release is corrupted. Expected '-rc1', got '-rc{}'!",
                            rc
                        ));
                        return Err(());
                    }
                } else {
                    // expected an RC and didn't get one! Something is wrong!
                    errorln!(format!(
                        "Started release is not a release candidate. Got '{}'.",
                        ver
                    ));
                    return Err(());
                }
            }
            Err(err) => {
                errorln!(format!("Unable to start v{}-rc1: {}", version, err));
                return Err(());
            }
        };

        Ok(())
    }

    /// Prepare release branches by creating them if necessary.
    ///
    fn create_release_branches(self: &Self, version: &Version) -> Result<bool, ()> {
        let mut res = false;
        // check whether we need to cut branches for each repository
        match self.maybe_cut_branches(&version) {
            Ok(None) => {
                log::info!("Branches ready to start release!");
            }
            Ok(Some(repos)) => {
                match self.cut_branches_for(&version, &repos) {
                    Ok(()) => {
                        log::info!("Success cutting branches for v{}", version);
                        res = true;
                    }
                    Err(_) => {
                        log::error!("Error cutting branches for v{}", version);
                        return Err(());
                    }
                };
            }
            Err(err) => {
                log::error!("Unable to cut branches for release {}: {}", version, err);
                return Err(());
            }
        };

        Ok(res)
    }

    fn maybe_cut_branches(
        self: &Self,
        version: &Version,
    ) -> Result<Option<Vec<&Repository>>, ReleaseError> {
        let repos = self.ws.repos.as_list();
        let base_version = version.get_base_version();
        let base_version_id = base_version.get_version_id();

        let mut repos_to_cut: Vec<&Repository> = vec![];
        for repo in &repos {
            let branches = match repo.get_release_branches() {
                Ok(v) => v,
                Err(()) => {
                    log::error!("unable to obtain branches for release");
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

        println!(
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
    fn cut_branches_for(
        self: &Self,
        version: &Version,
        repos: &Vec<&Repository>,
    ) -> Result<(), ReleaseError> {
        for repo in repos {
            log::info!("cut branch for repository {}", repo.name);
            match repo.branch_from_default(&version) {
                Ok(()) => {
                    log::info!("branched off!");
                }
                Err(()) => {
                    log::error!("error branching off!");
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
    fn start_release_candidate(
        self: &Self,
        notes: Option<&PathBuf>,
    ) -> Result<Version, ReleaseError> {
        // obtain current release version. Not having one would be quite unexpected.
        let relver = match &self.state {
            None => {
                log::error!("Release not started!");
                return Err(ReleaseError::NotStartedError);
            }
            Some(v) => &v.release_version,
        };

        // figure out which rc comes next.
        let avail_versions = self.get_release_versions(&relver);
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

        log::info!("Start next release candidate '{}': {}", next_rc, next_ver);

        // start release candidate on the various repositories, except
        // 's3gw.git'.
        let mut submodules = vec![
            SubmoduleInfo {
                name: "ui".into(),
                repo: &self.ws.repos.ui,
                tag_oid: None,
                tag_name: None,
                push_rc_tags: true,
            },
            SubmoduleInfo {
                name: "charts".into(),
                repo: &self.ws.repos.charts,
                tag_oid: None,
                tag_name: None,
                push_rc_tags: true,
            },
            SubmoduleInfo {
                name: "ceph".into(),
                repo: &self.ws.repos.ceph,
                tag_oid: None,
                tag_name: None,
                push_rc_tags: true,
            },
        ];

        for entry in &mut submodules {
            log::info!(
                "Tagging repository '{}' with version '{}'",
                entry.repo.name,
                next_ver
            );
            let (tag_name, tag_oid) = match entry.repo.tag_release_branch(&relver, &next_ver) {
                Ok((tag_name, tag_oid)) => {
                    log::info!(
                        "Tagged version '{}' with '{}' oid {} name {}",
                        relver,
                        next_ver,
                        tag_oid,
                        tag_name,
                    );
                    (tag_name, tag_oid)
                }
                Err(()) => {
                    log::error!("Error tagging version '{}' with '{}'", relver, next_ver);
                    return Err(ReleaseError::UnknownError);
                }
            };
            entry.tag_oid = Some(tag_oid);
            entry.tag_name = Some(tag_name);
        }

        // repositories have been tagged -- push them out so we can update the
        // submodules on 's3gw.git'.
        for entry in &submodules {
            log::info!("Pushing '{}' to repository '{}'", relver, entry.name);
            match entry.repo.push_release_branch(&relver) {
                Ok(()) => {
                    log::info!("Pushed '{}' to repository '{}'", relver, entry.name);
                }
                Err(()) => {
                    log::error!("Error pushing '{}' to repository '{}'!", relver, entry.name);
                    return Err(ReleaseError::UnknownError);
                }
            };

            if !entry.push_rc_tags {
                continue;
            }

            match entry.repo.push_release_tag(&next_ver) {
                Ok(()) => {
                    log::info!("Pushed '{}' to repository '{}'!", next_ver, entry.name);
                }
                Err(()) => {
                    log::error!(
                        "Error pushing '{}' to repository '{}'!",
                        next_ver,
                        entry.name
                    );
                    return Err(ReleaseError::UnknownError);
                }
            };
        }

        let mut paths_to_add: Vec<PathBuf> = vec![];

        // update submodules on 's3gw.git' to reflect the current state of each
        // repository.
        for entry in &submodules {
            let tag_name = match &entry.tag_name {
                None => {
                    log::error!("Tag name for submodule '{}' not set!", entry.name);
                    return Err(ReleaseError::UnknownError);
                }
                Some(n) => n,
            };
            let path = match self
                .ws
                .repos
                .s3gw
                .set_submodule_head(&entry.name, &tag_name, true)
            {
                Ok(p) => {
                    log::info!("Updated submodule '{}'", entry.name);
                    p
                }
                Err(()) => {
                    log::error!("Error updating submodule '{}'", entry.name);
                    return Err(ReleaseError::UnknownError);
                }
            };
            paths_to_add.push(path);
        }

        if let Some(notes_file) = notes {
            // copy release notes file to its final destination.
            let release_notes_file = format!("s3gw-v{}.md", next_ver);
            let release_notes_path = PathBuf::from("docs/release-notes").join(release_notes_file);
            let release_file_path = self.ws.repos.s3gw.path.join(&release_notes_path);

            match std::fs::copy(&notes_file, &release_file_path) {
                Ok(_) => {}
                Err(err) => {
                    boomln!(format!(
                        "Error copying notes file from '{}' to '{}': {}",
                        notes_file.display(),
                        release_file_path.display(),
                        err
                    ));
                }
            };
            paths_to_add.push(release_notes_path);
        }

        match self.ws.repos.s3gw.stage_paths(&paths_to_add) {
            Ok(()) => {
                log::info!(
                    "Staged paths:\n{}",
                    paths_to_add
                        .iter()
                        .map(|e| e.display().to_string())
                        .collect::<Vec<String>>()
                        .join("\n")
                );
            }
            Err(()) => {
                log::error!("Error staging paths!");
                return Err(ReleaseError::UnknownError);
            }
        };

        match self.ws.repos.s3gw.commit_release(&relver, &next_ver) {
            Ok(()) => {
                log::info!("Committed release '{}' tag '{}'", relver, next_ver);
            }
            Err(()) => {
                log::error!("Unable to commit release '{}' tag '{}'", relver, next_ver);
            }
        };

        // finally, push the branch and the release tag.
        match self.ws.repos.s3gw.push_release_branch(&relver) {
            Ok(()) => {
                log::info!("Pushed s3gw release branch for '{}'", relver);
            }
            Err(()) => {
                log::error!("Error pushing s3gw release branch for '{}'", relver);
                return Err(ReleaseError::UnknownError);
            }
        };

        match self.ws.repos.s3gw.push_release_tag(&next_ver) {
            Ok(()) => {
                log::info!(
                    "Pushed s3gw release tag '{}' for version '{}'",
                    next_ver,
                    relver
                );
            }
            Err(()) => {
                log::error!(
                    "Error pushing s3gw release tag '{}' for version '{}'",
                    next_ver,
                    relver
                );
                return Err(ReleaseError::UnknownError);
            }
        };

        log::info!("Started release ver '{}' tag '{}'", relver, next_ver);
        Ok(next_ver)
    }
}
