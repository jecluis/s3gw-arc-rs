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

use std::{collections::HashMap, path::PathBuf};

use crate::git;

use super::config::{WSGitRepoConfigValues, WSGitReposConfig, WSUserConfig};

pub struct Repository {
    pub name: String,
    pub path: PathBuf,
    pub user_config: WSUserConfig,
    pub config: WSGitRepoConfigValues,
}

pub struct Repos {
    pub s3gw: Repository,
    pub ui: Repository,
    pub charts: Repository,
    pub ceph: Repository,
}

impl Repos {
    pub fn init(
        base_path: &PathBuf,
        user_config: &WSUserConfig,
        git_config: &WSGitReposConfig,
    ) -> Result<Repos, ()> {
        let s3gw = match Repository::init(
            &"s3gw".into(),
            &base_path.join("s3gw.git"),
            &user_config,
            &git_config.s3gw,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let ui = match Repository::init(
            &"s3gw-ui".into(),
            &base_path.join("s3gw-ui.git"),
            &user_config,
            &git_config.ui,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let charts = match Repository::init(
            &"s3gw-charts".into(),
            &base_path.join("charts.git"),
            &user_config,
            &git_config.s3gw,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let ceph = match Repository::init(
            &"s3gw-ceph".into(),
            &base_path.join("ceph.git"),
            &user_config,
            &git_config.s3gw,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };

        Ok(Repos {
            s3gw,
            ui,
            charts,
            ceph,
        })
    }

    pub fn as_list(self: &Self) -> Vec<&Repository> {
        vec![&self.s3gw, &self.ui, &self.charts, &self.ceph]
    }
}

impl Repository {
    pub fn init(
        name: &String,
        path: &PathBuf,
        user_config: &WSUserConfig,
        config: &WSGitRepoConfigValues,
    ) -> Result<Repository, ()> {
        let repo = Repository {
            name: name.clone(),
            path: path.to_path_buf(),
            user_config: user_config.clone(),
            config: config.clone(),
        };
        Ok(repo)
    }

    /// Synchronize local repository with its upstream. If the repository does
    /// not exist yet, it will be cloned.
    ///
    pub fn sync<T>(self: &Self, mut progress_cb: T) -> Result<(), ()>
    where
        T: FnMut(&str, u64, u64),
    {
        if !self.path.exists() {
            // clone repository
            let git = match git::repo::GitRepo::clone(
                &self.path,
                &self.config.readonly,
                &self.config.readwrite,
                |n: u64, total: u64| {
                    progress_cb("clone", n, total);
                },
            ) {
                Ok(v) => v,
                Err(_) => return Err(()),
            };
            // init submodules

            // set config values
            git.set_user_name(&self.user_config.name)
                .set_user_email(&self.user_config.email)
                .set_signing_key(&self.user_config.signing_key);
        }
        // git remote update
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        log::debug!("Updating remote for repo at {}", self.path.display());
        match git.remote_update() {
            Ok(_) => {
                log::debug!("Updated remote");
            }
            Err(_) => {
                log::debug!("Error updating remote");
                return Err(());
            }
        };

        Ok(())
    }

    pub fn get_release_versions(self: &Self) -> Result<Vec<super::version::ReleaseVersion>, ()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };
        let refs = match git.get_refs() {
            Ok(v) => v,
            Err(_) => {
                log::error!(
                    "Unable to obtain refs for repository '{}'",
                    self.path.display()
                );
                return Err(());
            }
        };

        let mut versions: HashMap<String, Vec<super::version::Version>> = HashMap::new();

        let branch_re = regex::Regex::new(&self.config.branch_pattern).expect(
            format!(
                "potentially malformed branch pattern '{}'",
                self.config.branch_pattern
            )
            .as_str(),
        );
        let tag_re = regex::Regex::new(&self.config.tag_pattern).expect(
            format!(
                "potentially malformed tag pattern '{}'",
                self.config.tag_pattern
            )
            .as_str(),
        );

        for branch in refs.branches {
            log::trace!("branch '{}' oid {}", branch.name, branch.oid);
            if let Some(m) = branch_re.captures(&branch.name) {
                assert_eq!(m.len(), 2);
                log::trace!("  matches");

                let version = match m.get(1) {
                    None => {
                        continue;
                    }
                    Some(v) => v,
                };
                let version_str = String::from(version.as_str());
                log::trace!("  version: {}", version_str);
                assert!(!versions.contains_key(&version_str));

                versions.insert(version_str, vec![]);
            }
        }

        for tag in refs.tags {
            log::trace!("tag '{}' oid {}", tag.name, tag.oid);
            if let Some(m) = tag_re.captures(&tag.name) {
                assert_eq!(m.len(), 2);
                log::trace!("  matches");

                let version_raw = match m.get(1) {
                    None => {
                        continue;
                    }
                    Some(v) => v,
                };
                let version_str = String::from(version_raw.as_str());
                log::trace!("  version: {}", version_str);

                let version = match super::version::Version::from_str(&version_str) {
                    Ok(v) => v,
                    Err(_) => {
                        log::debug!("Unable to parse version from '{}' - skip.", version_str);
                        continue;
                    }
                };

                let relversion_str = version.get_release_version_str();
                if !versions.contains_key(&relversion_str) {
                    log::trace!(
                        "Unable to find release '{}' for version '{}'",
                        relversion_str,
                        version_str
                    );
                    continue;
                }
                let rel = versions.get_mut(&relversion_str).unwrap();
                rel.push(version);
                log::trace!("  added to release {}", relversion_str);
            }
        }

        let mut res: Vec<super::version::ReleaseVersion> = vec![];
        for entry in versions {
            let rel_str = entry.0;
            let rel_ver = match super::version::Version::from_str(&rel_str) {
                Ok(v) => v,
                Err(_) => {
                    log::error!("Unable to parse version from '{}'", rel_str);
                    return Err(());
                }
            };
            let mut version_vec = entry.1.to_vec();
            version_vec.sort_by_key(|e: &super::version::Version| e.get_version_id());
            res.push(super::version::ReleaseVersion {
                release_version: rel_ver,
                versions: version_vec,
            });
        }
        res.sort_by_key(|e: &super::version::ReleaseVersion| e.release_version.get_version_id());

        Ok(res)
    }
}