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
    release::{errors::ReleaseError, ReleaseState},
    ws::{repository::Repository, version::Version},
};

use super::Release;

impl Release {
    pub fn start(self: &mut Self, version: &Version, notes: &PathBuf) -> Result<(), ()> {
        // 1. sync rw repos to force authorized connect
        // 2. check all repos for existing versions
        // 2.1. make sure this version has not been started in any of the
        //      existing repositories.
        // 3. start release procedures.

        if let Some(s) = &self.state {
            println!("On-going release detected!");
            if &s.release_version == version {
                println!("  Maybe you want to 'continue' instead?");
            } else {
                println!(
                    "  Detected version {}, attempting to start {}!",
                    s.release_version, version
                );
            }
            return Err(());
        }

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
            println!("version {} has already been released.", version);
            return Err(());
        }

        // NOTE(joao): we should check whether there is a started release across
        // the repositories. This can be done by checking for rc versions on
        // every repository. For now we will ignore this bit.

        if avail_it.count() > 0 {
            println!("release version {} has already been started.", version);
            return Err(());
        }

        log::info!("start releasing version {}", version);

        match self.create_release_branches(&version) {
            Ok(true) => {
                println!("Created release branches.");
            }
            Ok(false) => {
                println!("Release branches already exist.");
            }
            Err(()) => {
                log::error!("Error creating release!");
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
                log::error!("Unable to write release state file!");
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
                        log::error!("Release is corrupted. Expected '-rc1', got '-rc{}'!", rc);
                        return Err(());
                    }
                } else {
                    // expected an RC and didn't get one! Something is wrong!
                    log::error!("Started release is not a release candidate. Got '{}'.", ver);
                    return Err(());
                }
            }
            Err(err) => {
                log::error!("Unable to start v{}-rc1: {}", version, err);
                return Err(());
            }
        };

        self.ws.repos.s3gw.print_version_tree();

        self.ws.repos.s3gw.test_ssh();
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

        self.ws.repos.s3gw.tmp_get_refs();

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
        let repos = vec![
            &self.ws.repos.ui,
            &self.ws.repos.charts,
            &self.ws.repos.ceph,
        ];

        for repo in repos {}

        Err(ReleaseError::UnknownError)
    }
}
