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
    path::PathBuf,
};

use crate::git::{self, refs::GitRefEntry};

use super::{
    config::{WSGitRepoConfigValues, WSGitReposConfig, WSUserConfig},
    version::Version,
};

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
            &git_config.charts,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let ceph = match Repository::init(
            &"s3gw-ceph".into(),
            &base_path.join("ceph.git"),
            &user_config,
            &git_config.ceph,
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
    pub fn sync(self: &Self, sync_submodules: bool) -> Result<(), ()> {
        if !self.path.exists() {
            // clone repository
            let git = match git::repo::GitRepo::clone(
                &self.path,
                &self.config.readonly,
                &self.config.readwrite,
                &self.name,
            ) {
                Ok(v) => v,
                Err(_) => return Err(()),
            };
            // init submodules

            // set config values
            git.set_user_name(&self.user_config.name)
                .set_user_email(&self.user_config.email)
                .set_signing_key(&self.user_config.signing_key);

            if sync_submodules {
                match git.submodules_update() {
                    Ok(()) => {
                        log::debug!("Updated submodules for cloned repo");
                    }
                    Err(()) => {
                        log::error!("Error updating submodules for cloned repo!");
                        return Err(());
                    }
                };
            }
        }
        // git remote update
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        log::debug!("Updating remote for repo at {}", self.path.display());
        match git.remote_update(&self.name) {
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

        let branch_refs: Vec<&git::refs::GitRefEntry> =
            refs.iter().filter(|e| e.is_branch()).collect();
        let tag_refs: Vec<&git::refs::GitRefEntry> = refs.iter().filter(|e| e.is_tag()).collect();

        log::trace!("--------- branches ----------");
        log::trace!(" total: {}", branch_refs.len());
        for branch in &branch_refs {
            log::trace!(
                " > {} (oid: {}), remote: {}",
                branch.name,
                branch.oid,
                branch.is_remote
            );
        }

        for branch in branch_refs {
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
                if !versions.contains_key(&version_str) {
                    versions.insert(version_str, vec![]);
                }
            }
        }

        for tag in tag_refs {
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

                let relversion_str = version.get_base_version_str();
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

    pub fn get_version_tree(self: &Self) -> Result<BTreeMap<u64, super::version::BaseVersion>, ()> {
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

        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };
        let refs = match git.get_refs() {
            Ok(v) => v,
            Err(()) => {
                log::error!(
                    "Unable to obtain refs for repository at '{}'",
                    self.path.display()
                );
                return Err(());
            }
        };

        let mut version_tree: BTreeMap<u64, super::version::BaseVersion> = BTreeMap::new();
        let branch_refs: Vec<&git::refs::GitRefEntry> =
            refs.iter().filter(|e| e.is_branch()).collect();
        let tag_refs: Vec<&git::refs::GitRefEntry> = refs.iter().filter(|e| e.is_tag()).collect();

        for branch in branch_refs {
            log::trace!("branch '{}' oid {}", branch.name, branch.oid);
            if let Some(m) = branch_re.captures(&branch.name) {
                assert_eq!(m.len(), 2);

                let version = if let Some(v) = m.get(1) {
                    super::version::Version::from_str(&String::from(v.as_str())).unwrap()
                } else {
                    log::trace!("  not a match - skip.");
                    continue;
                };
                let verid = version.get_version_id();
                assert!(!version_tree.contains_key(&verid));
                version_tree.insert(
                    verid,
                    crate::ws::version::BaseVersion {
                        version,
                        releases: BTreeMap::new(),
                    },
                );
            }
        }

        for tag in tag_refs {
            log::trace!("tag '{}' oid {}", tag.name, tag.oid);
            if let Some(m) = tag_re.captures(&tag.name) {
                assert_eq!(m.len(), 2);

                let version = if let Some(v) = m.get(1) {
                    match super::version::Version::from_str(&String::from(v.as_str())) {
                        Ok(ver) => ver,
                        Err(()) => {
                            log::debug!("unable to parse version '{}' - skip.", v.as_str());
                            continue;
                        }
                    }
                } else {
                    continue;
                };

                let base_ver = version.get_base_version();
                let base_ver_id = base_ver.get_version_id();
                if !version_tree.contains_key(&base_ver_id) {
                    log::trace!(
                        "base version {} for {} not found - skip.",
                        base_ver,
                        version
                    );
                    continue;
                }

                let base_version = version_tree.get_mut(&base_ver_id).unwrap();
                let release_ver = version.get_release_version();
                let release_ver_id = release_ver.get_version_id();
                if !base_version.releases.contains_key(&release_ver_id) {
                    base_version.releases.insert(
                        release_ver_id,
                        crate::ws::version::ReleaseDesc {
                            release: release_ver.clone(),
                            versions: BTreeMap::new(),
                            is_complete: false,
                        },
                    );
                }

                let version_desc_tree = base_version.releases.get_mut(&release_ver_id).unwrap();
                let version_id = version.get_version_id();
                if version.rc.is_none() {
                    version_desc_tree.is_complete = true;
                }
                version_desc_tree.versions.insert(version_id, version);
            }
        }

        Ok(version_tree)
    }

    pub fn print_version_tree(self: &Self) {
        let tree = match self.get_version_tree() {
            Ok(t) => t,
            Err(()) => {
                log::error!("Unable to print version tree for '{}'", self.name);
                return;
            }
        };

        for base_version in tree.values() {
            println!("v{}", base_version.version);
            for release_desc in base_version.releases.values() {
                println!(
                    "  - v{} ({})",
                    release_desc.release,
                    match release_desc.is_complete {
                        true => "complete",
                        false => "incomplete",
                    }
                );
                for version in release_desc.versions.values() {
                    println!("    - v{}", version);
                }
            }
        }
    }

    fn get_versions_from_refs(
        self: &Self,
        refs: &Vec<&crate::git::refs::GitRefEntry>,
        regex_pattern: &String,
    ) -> Result<BTreeMap<u64, Version>, ()> {
        let regex = match regex::Regex::new(&regex_pattern) {
            Ok(r) => r,
            Err(e) => {
                log::error!("potentially malformed pattern '{}': {}", regex_pattern, e);
                return Err(());
            }
        };

        let mut versions: BTreeMap<u64, Version> = BTreeMap::new();
        for entry in refs {
            log::trace!(
                "get_versions_from_refs: process '{}' (oid {})",
                entry.name,
                entry.oid
            );
            if let Some(m) = regex.captures(&entry.name) {
                assert_eq!(m.len(), 2);

                let version = if let Some(v) = m.get(1) {
                    match Version::from_str(&String::from(v.as_str())) {
                        Ok(r) => r,
                        Err(()) => {
                            log::trace!("malformed version '{}' - skip.", v.as_str());
                            continue;
                        }
                    }
                } else {
                    log::trace!("  not a match - skip.");
                    continue;
                };
                let version_id = version.get_version_id();
                log::trace!(
                    "version id {} for ref {} ({})",
                    version_id,
                    entry.name,
                    match entry.is_remote {
                        true => "remote",
                        false => "local",
                    }
                );
                if !versions.contains_key(&version_id) {
                    versions.insert(version_id, version);
                }
            }
        }

        Ok(versions)
    }

    fn get_git_refs(self: &Self) -> Result<Vec<crate::git::refs::GitRefEntry>, ()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!("unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };
        git.get_refs()
    }

    pub fn get_versions(self: &Self) -> Result<BTreeMap<u64, Version>, ()> {
        let refs = match self.get_git_refs() {
            Ok(v) => v,
            Err(()) => {
                log::error!(
                    "unable to obtain refs for repository at '{}'",
                    self.path.display()
                );
                return Err(());
            }
        };

        let tag_refs: Vec<&git::refs::GitRefEntry> = refs.iter().filter(|e| e.is_tag()).collect();

        match self.get_versions_from_refs(&tag_refs, &self.config.tag_pattern) {
            Ok(v) => Ok(v),
            Err(()) => {
                log::error!(
                    "unable to obtain versions from refs from repository at '{}'",
                    self.path.display()
                );
                return Err(());
            }
        }
    }

    pub fn get_release_branches(self: &Self) -> Result<BTreeMap<u64, Version>, ()> {
        let refs = match self.get_git_refs() {
            Ok(v) => v,
            Err(()) => {
                log::error!(
                    "unable to obtain refs for repository '{}'",
                    self.path.display()
                );
                return Err(());
            }
        };

        let branch_refs: Vec<&crate::git::refs::GitRefEntry> =
            refs.iter().filter(|e| e.is_branch()).collect();

        match self.get_versions_from_refs(&branch_refs, &self.config.branch_pattern) {
            Ok(v) => Ok(v),
            Err(()) => {
                log::error!(
                    "unable to obtain branches from refs from repository at '{}'",
                    self.path.display()
                );
                return Err(());
            }
        }
    }

    pub fn _find_version(self: &Self, version: &super::version::Version) -> Result<(), ()> {
        let ver_id = version.get_version_id();
        let base_ver = version.get_base_version();
        let base_ver_id = base_ver.get_version_id();

        log::trace!(
            "find version {} (id {}), base {} (id {})",
            version,
            ver_id,
            base_ver,
            base_ver_id
        );

        let versions = match self.get_versions() {
            Ok(v) => v,
            Err(()) => {
                log::error!("unable to obtain versions!");
                return Err(());
            }
        };

        match versions.get(&ver_id) {
            Some(_) => Ok(()),
            None => Err(()),
        }
    }

    pub fn test_ssh(self: &Self) {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!(
                    "Unable to open git repository at '{}' to test ssh!",
                    self.path.display()
                );
                return;
            }
        };

        git.test_ssh();
    }

    pub fn branch_from_default(self: &Self, dst: &Version) -> Result<(), ()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };

        let dst_branch = dst.to_str_fmt(&self.config.branch_format);
        match git.branch_from_default(&dst_branch) {
            Ok(()) => {
                log::info!("success!");
            }
            Err(()) => {
                log::error!("error!");
                return Err(());
            }
        }

        Ok(())
    }

    pub fn _find_release_branch(self: &Self, relver: &Version) -> Result<(), ()> {
        let refs = match self.get_git_refs() {
            Ok(r) => r,
            Err(()) => return Err(()),
        };

        let branch_name = relver.to_str_fmt(&self.config.branch_format);
        let branch_refs: Vec<&GitRefEntry> = refs
            .iter()
            .filter(|e| e.is_branch() && e.name == branch_name)
            .collect();

        if branch_refs.len() == 0 {
            log::error!(
                "Unable to find release branch for '{}' in repo '{}'!",
                branch_name,
                self.name
            );
            return Err(());
        }

        let is_remote = branch_refs.iter().any(|e| e.is_remote);

        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };

        let branch = match git.find_branch(&branch_name, &is_remote) {
            Ok(b) => b,
            Err(()) => {
                log::error!("Error obtaining branch '{}'", branch_name);
                return Err(());
            }
        };

        Ok(())
    }

    pub fn tag_release_branch(
        self: &Self,
        relver: &Version,
        tagver: &Version,
    ) -> Result<(String, String), ()> {
        let branch_name = relver.to_str_fmt(&self.config.branch_format);
        let base_tag_name = tagver.to_str_fmt(&self.config.tag_format);
        let tag_name = if let Some(rc) = tagver.rc {
            format!("{}-rc{}", base_tag_name, rc)
        } else {
            base_tag_name.clone()
        };

        let patchver = if let Some(v) = &relver.patch {
            v
        } else {
            return Err(());
        };
        let tagver_str = format!("v{}.{}.{}", &relver.major, &relver.minor, &patchver);
        let tag_msg = match tagver.rc {
            Some(rc) => {
                format!("release candidate {} for {}", rc, tagver_str)
            }
            None => {
                format!("release {}", tagver_str)
            }
        };

        // We use the 'git' command here because we have yet to find a library
        // that will allow us to do signed annotated tags. Also, we get the
        // additional benefit of having it dealing with the GPG key handling for us.
        match std::process::Command::new("git")
            .args([
                "-C",
                self.path.to_str().unwrap(),
                "tag",
                "--annotate",
                "--sign",
                "-m",
                tag_msg.as_str(),
                tag_name.as_str(),
                branch_name.as_str(),
            ])
            .status()
        {
            Ok(res) => {
                if !res.success() {
                    log::error!(
                        "Unable to tag '{}' with '{}': {}",
                        branch_name,
                        tag_name,
                        res.code().unwrap()
                    );
                    return Err(());
                }
            }
            Err(err) => {
                log::error!("Unable to run 'git' command: {}", err);
                return Err(());
            }
        };

        let (tag_oid, commit_oid) =
            match self.get_sha1_by_refspec(&format!("refs/tags/{}", tag_name)) {
                Ok(s) => s,
                Err(()) => {
                    log::error!("Unable to obtain sha1 for tag '{}'", tag_name);
                    return Err(());
                }
            };

        log::info!(
            "Tagged {} with {} oid {} commit {}",
            branch_name,
            tag_name,
            tag_oid,
            commit_oid,
        );

        Ok((tag_oid, commit_oid))
    }

    fn get_sha1_by_refspec(self: &Self, refspec: &String) -> Result<(String, String), ()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };
        let res = match git.get_oid_by_refspec(refspec) {
            Ok(obj) => {
                let oid = obj.id().to_string();
                let commit = match obj.peel_to_commit() {
                    Err(err) => {
                        log::error!("Enable to find commit for refspec '{}': {}", refspec, err);
                        return Err(());
                    }
                    Ok(c) => c.id().to_string(),
                };
                Ok((oid, commit))
            }
            Err(()) => Err(()),
        };
        res
    }

    pub fn push(self: &Self, refspec: &String) -> Result<(), ()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(());
            }
        };
        match git.push(&refspec) {
            Ok(()) => {
                log::info!("Pushed '{}'!", refspec);
                Ok(())
            }
            Err(()) => {
                log::error!("Error pushing '{}'!", refspec);
                Err(())
            }
        }
    }

    pub fn push_release_branch(self: &Self, relver: &Version) -> Result<(), ()> {
        let relver_str = relver.to_str_fmt(&self.config.branch_format);
        let refspec = format!("refs/heads/{}", relver_str);
        self.push(&refspec)
    }

    pub fn push_release_tag(self: &Self, tagver: &Version) -> Result<(), ()> {
        let tagver_base_str = tagver.to_str_fmt(&self.config.tag_format);
        let tagver_str = if let Some(rc) = tagver.rc {
            format!("{}-rc{}", tagver_base_str, rc)
        } else {
            tagver_base_str
        };
        let refspec = format!("refs/tags/{}", tagver_str);
        self.push(&refspec)
    }
}
