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

use std::{collections::BTreeMap, path::PathBuf};

use crate::git;
use crate::{boomln, version::Version};
use crate::{errorln, successln};

use super::errors::RepositoryResult;
use super::{
    config::{WSGitRepoConfigValues, WSGitReposConfig, WSUserConfig},
    errors::RepositoryError,
};

#[derive(Clone)]
pub struct Repository {
    pub name: String,
    pub path: PathBuf,
    pub user_config: WSUserConfig,
    pub config: WSGitRepoConfigValues,
    pub update_submodules: bool,
}

#[derive(Clone)]
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
            true,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let ui = match Repository::init(
            &"s3gw-ui".into(),
            &base_path.join("s3gw-ui.git"),
            &user_config,
            &git_config.ui,
            false,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let charts = match Repository::init(
            &"s3gw-charts".into(),
            &base_path.join("charts.git"),
            &user_config,
            &git_config.charts,
            false,
        ) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let ceph = match Repository::init(
            &"s3gw-ceph".into(),
            &base_path.join("ceph.git"),
            &user_config,
            &git_config.ceph,
            false,
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

    pub fn as_vec(self: &Self) -> Vec<&Repository> {
        vec![&self.s3gw, &self.ui, &self.charts, &self.ceph]
    }
}

impl Repository {
    pub fn init(
        name: &String,
        path: &PathBuf,
        user_config: &WSUserConfig,
        config: &WSGitRepoConfigValues,
        update_submodules: bool,
    ) -> Result<Repository, ()> {
        let repo = Repository {
            name: name.clone(),
            path: path.to_path_buf(),
            user_config: user_config.clone(),
            config: config.clone(),
            update_submodules,
        };
        Ok(repo)
    }

    fn version_to_str(self: &Self, ver: &Version, is_tag: bool) -> String {
        log::trace!(
            "version_to_str: repo name '{}' path '{}' format '{}'",
            self.name,
            self.path.display(),
            if is_tag {
                &self.config.tag_format
            } else {
                &self.config.branch_format
            }
        );
        let ver_base_str = ver.to_str_fmt(if is_tag {
            &self.config.tag_format
        } else {
            &self.config.branch_format
        });
        log::trace!(
            "version_to_str: base str '{}' ver '{}' is_tag {}",
            ver_base_str,
            ver,
            is_tag
        );
        if let Some(rc) = ver.rc {
            assert!(is_tag);
            format!("{}-rc{}", ver_base_str, rc)
        } else {
            ver_base_str
        }
    }

    /// Synchronize local repository with its upstream. If the repository does
    /// not exist yet, it will be cloned.
    ///
    pub fn sync(self: &Self, sync_submodules: bool) -> RepositoryResult<()> {
        if !self.path.exists() {
            // clone repository
            let git = match git::repo::GitRepo::clone(
                &self.path,
                &self.config.readonly,
                &self.config.readwrite,
                &self.name,
            ) {
                Ok(v) => v,
                Err(()) => return Err(RepositoryError::UnknownError),
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
            Err(()) => return Err(RepositoryError::UnableToOpenRepositoryError),
        };
        log::debug!("Updating remote for repo at {}", self.path.display());
        match git.remote_update(&self.name) {
            Ok(()) => {
                log::debug!("Updated remote");
            }
            Err(()) => {
                log::debug!("Error updating remote");
                return Err(RepositoryError::RemoteUpdateError);
            }
        };

        if sync_submodules {
            match git.submodules_update() {
                Ok(()) => {
                    log::debug!("Updated submodules for repo");
                }
                Err(()) => {
                    log::error!("Error updating submodules for repo!");
                    return Err(RepositoryError::SubmoduleUpdateError);
                }
            };
        }

        Ok(())
    }

    /// Obtain releases. Returns a tree ordered by release ID, each value
    /// referring to a base release version (i.e., 0.99), containing another
    /// tree of release versions (i.e., 0.99.0) associated with said base version.
    /// Each release version entry will have an associated tree of versions
    /// (i.e., 0.99.0, 0.99.0-rc1, ...).
    ///
    pub fn get_releases(
        self: &Self,
    ) -> RepositoryResult<BTreeMap<u64, crate::version::BaseVersion>> {
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
                boomln!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        let refs = match git.get_refs() {
            Ok(v) => v,
            Err(()) => {
                log::error!(
                    "Unable to obtain refs for repository at '{}'",
                    self.path.display()
                );
                return Err(RepositoryError::UnableToGetReferencesError);
            }
        };

        let mut version_tree: BTreeMap<u64, crate::version::BaseVersion> = BTreeMap::new();
        let branch_refs: Vec<&git::refs::GitRef> =
            refs.values().filter(|e| e.is_branch()).collect();
        let tag_refs: Vec<&git::refs::GitRef> = refs.values().filter(|e| e.is_tag()).collect();

        // populate tree with all the existing releases -- i.e., all the git
        // refs that match this repository's branch pattern.
        for branch in branch_refs {
            log::trace!("get_releases: branch '{}'", branch.name);
            if let Some(m) = branch_re.captures(&branch.name) {
                assert_eq!(m.len(), 2);

                let version = if let Some(v) = m.get(1) {
                    Version::from_str(&String::from(v.as_str())).unwrap()
                } else {
                    log::trace!("  not a match - skip.");
                    continue;
                };
                let verid = version.get_version_id();
                if !version_tree.contains_key(&verid) {
                    version_tree.insert(
                        verid,
                        crate::version::BaseVersion {
                            version,
                            releases: BTreeMap::new(),
                        },
                    );
                }
            }
        }

        // for each git ref matching this repository's tag format, add it to the
        // corresponding release entry. Skip a tag if its expected release is
        // not found in the version tree.
        for tag in tag_refs {
            log::trace!("get_releases: tag '{}'", tag.name);
            if let Some(m) = tag_re.captures(&tag.name) {
                assert_eq!(m.len(), 2);

                let version = if let Some(v) = m.get(1) {
                    match Version::from_str(&String::from(v.as_str())) {
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
                        crate::version::ReleaseEntry {
                            release: release_ver.clone(),
                            versions: BTreeMap::new(),
                            is_complete: false,
                        },
                    );
                }

                let version_entry_tree = base_version.releases.get_mut(&release_ver_id).unwrap();
                let version_id = version.get_version_id();
                if version.rc.is_none() {
                    version_entry_tree.is_complete = true;
                }
                version_entry_tree.versions.insert(version_id, version);
            }
        }

        Ok(version_tree)
    }

    pub fn _print_version_tree(self: &Self) {
        let tree = match self.get_releases() {
            Ok(t) => t,
            Err(err) => {
                log::error!("Unable to print version tree for '{}': {}", self.name, err);
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

    /// Obtain all versions from the provided vector of references, putting
    /// each reference through a regular expression for parsing to obtain the version.
    ///
    fn get_versions_from_refs(
        self: &Self,
        refs: &Vec<&crate::git::refs::GitRef>,
        regex_pattern: &String,
    ) -> RepositoryResult<BTreeMap<u64, Version>> {
        let regex = match regex::Regex::new(&regex_pattern) {
            Ok(r) => r,
            Err(e) => {
                log::error!("potentially malformed pattern '{}': {}", regex_pattern, e);
                return Err(RepositoryError::UnknownError);
            }
        };

        let mut versions: BTreeMap<u64, Version> = BTreeMap::new();
        for entry in refs {
            log::trace!("get_versions_from_refs: handle '{}'", entry.name,);
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
                    match entry.has_remote {
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

    pub fn get_git_refs(self: &Self) -> RepositoryResult<crate::git::refs::GitRefMap> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!("unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        match git.get_refs() {
            Ok(m) => Ok(m),
            Err(()) => Err(RepositoryError::UnableToGetReferencesError),
        }
    }

    /// Obtain vector containing all the GitRefEntry that are branches, local or remote.
    ///
    pub fn get_heads_refs(self: &Self) -> RepositoryResult<Vec<crate::git::refs::GitRef>> {
        let refs = match self.get_git_refs() {
            Ok(r) => r,
            Err(err) => {
                boomln!(
                    "Unable to obtain git refs for repository '{}': {}",
                    self.name,
                    err
                );
                return Err(err);
            }
        };
        let mut heads = vec![];
        for entry in refs.values().filter(|e| e.is_branch()) {
            heads.push(entry.clone());
        }
        Ok(heads)
    }

    /// Obtain all versions, from tags, known to this repository.
    ///
    pub fn get_versions(self: &Self) -> RepositoryResult<BTreeMap<u64, Version>> {
        let refs = match self.get_git_refs() {
            Ok(v) => v,
            Err(err) => {
                log::error!(
                    "unable to obtain refs for repository at '{}': {}",
                    self.path.display(),
                    err
                );
                return Err(err);
            }
        };

        let tag_refs: Vec<&git::refs::GitRef> = refs.values().filter(|e| e.is_tag()).collect();

        match self.get_versions_from_refs(&tag_refs, &self.config.tag_pattern) {
            Ok(v) => Ok(v),
            Err(err) => {
                log::error!(
                    "unable to obtain versions from refs from repository at '{}': {}",
                    self.path.display(),
                    err
                );
                return Err(err);
            }
        }
    }

    /// Obtain all release branches known to this repository, both local and remote.
    ///
    pub fn get_release_branches(self: &Self) -> RepositoryResult<BTreeMap<u64, Version>> {
        let refs = match self.get_git_refs() {
            Ok(v) => v,
            Err(err) => {
                log::error!(
                    "unable to obtain refs for repository '{}': {}",
                    self.path.display(),
                    err
                );
                return Err(err);
            }
        };

        let branch_refs: Vec<&crate::git::refs::GitRef> =
            refs.values().filter(|e| e.is_branch()).collect();

        match self.get_versions_from_refs(&branch_refs, &self.config.branch_pattern) {
            Ok(v) => Ok(v),
            Err(err) => {
                log::error!(
                    "unable to obtain branches from refs from repository at '{}': {}",
                    self.path.display(),
                    err
                );
                return Err(err);
            }
        }
    }

    /// Create a new branch 'dst' from this repository's default branch.
    ///
    pub fn branch_from_default(self: &Self, dst: &Version) -> RepositoryResult<()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };

        let dst_branch = dst.to_str_fmt(&self.config.branch_format);
        match git.branch_from_default(&dst_branch) {
            Ok(()) => {
                log::info!("Success branching from default to '{}'!", dst_branch);
            }
            Err(()) => {
                log::error!("Error branching from default to '{}'!", dst_branch);
                return Err(RepositoryError::BranchingError);
            }
        }

        match git.checkout_branch(&dst_branch) {
            Ok(()) => {
                log::info!("Checked out '{}' on repository '{}'", dst_branch, self.name);
            }
            Err(()) => {
                log::error!(
                    "Unable to checkout '{}' on repository '{}'",
                    dst_branch,
                    self.name
                );
                return Err(RepositoryError::CheckoutError);
            }
        };

        Ok(())
    }

    /// Checks out the branch for the specified base version. Will fetch the
    /// branch if not already local. Will ensure branch is updated from remote
    /// branch.
    pub fn checkout_branch(self: &Self, base_ver: &Version) -> RepositoryResult<()> {
        let heads = match self.get_heads_refs() {
            Ok(v) => v,
            Err(err) => {
                return Err(err);
            }
        };
        let branch_str = self.version_to_str(&base_ver, false);
        let ref_entry = match heads.iter().find(|e| e.name == branch_str) {
            None => {
                errorln!("Unable to find branch '{}' local or remote", branch_str);
                return Err(RepositoryError::UnknownBranchError);
            }
            Some(e) => e,
        };

        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(v) => v,
            Err(()) => {
                boomln!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };

        if !ref_entry.has_local {
            // must fetch branch prior to checkout
            match git.fetch(&format!("refs/heads/{}", branch_str), &branch_str) {
                Ok(()) => {
                    successln!("Successfully fetched '{}'", branch_str);
                }
                Err(()) => {
                    errorln!("Error fetching '{}'", branch_str);
                    return Err(RepositoryError::FetchingError);
                }
            };
        }
        // checkout branch
        match git.checkout_branch(&branch_str) {
            Ok(()) => {
                log::info!("Checked out '{}' on repository '{}'", branch_str, self.name);
            }
            Err(()) => {
                log::error!(
                    "Unable to checkout '{}' on repository '{}'",
                    branch_str,
                    self.name
                );
                return Err(RepositoryError::CheckoutError);
            }
        };

        Ok(())
    }

    /// Create a new release version tag for a given release version. The branch
    /// to be tagged will be decided from the 'relver' provided, while the tag
    /// to tag it with will be derived from 'tagver'.
    ///
    pub fn tag_release_branch(
        self: &Self,
        relver: &Version,
        tagver: &Version,
    ) -> RepositoryResult<(String, String)> {
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
            panic!("Expected patch version on relver '{}'", relver);
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
                    return Err(RepositoryError::UnknownError);
                }
            }
            Err(err) => {
                log::error!("Unable to run 'git' command: {}", err);
                return Err(RepositoryError::UnknownError);
            }
        };

        let (tag_oid, commit_oid) =
            match self.get_sha1_by_refspec(&format!("refs/tags/{}", tag_name)) {
                Ok(s) => s,
                Err(err) => {
                    log::error!("Unable to obtain sha1 for tag '{}'", tag_name);
                    return Err(err);
                }
            };

        log::info!(
            "Tagged {} with {} oid {} commit {}",
            branch_name,
            tag_name,
            tag_oid,
            commit_oid,
        );

        Ok((tag_name, tag_oid))
    }

    /// Obtain a given refspec's SHA1.
    ///
    fn get_sha1_by_refspec(self: &Self, refspec: &String) -> RepositoryResult<(String, String)> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        let res = match git.get_oid_by_refspec(refspec) {
            Ok(obj) => {
                let oid = obj.id().to_string();
                let commit = match obj.peel_to_commit() {
                    Err(err) => {
                        log::error!("Enable to find commit for refspec '{}': {}", refspec, err);
                        return Err(RepositoryError::UnknownSHA1Error);
                    }
                    Ok(c) => c.id().to_string(),
                };
                Ok((oid, commit))
            }
            Err(()) => Err(RepositoryError::UnknownError),
        };
        res
    }

    /// Push the given refspec to this repository's read-write remote.
    ///
    pub fn push(self: &Self, refspec: &String) -> RepositoryResult<()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        match git.push(&refspec) {
            Ok(()) => {
                log::info!("Pushed '{}'!", refspec);
                Ok(())
            }
            Err(()) => {
                log::error!("Error pushing '{}'!", refspec);
                Err(RepositoryError::PushingError)
            }
        }
    }

    /// Push the provided 'relver' release version branch to this repository's
    /// read-write remote.
    ///
    pub fn push_release_branch(self: &Self, relver: &Version) -> RepositoryResult<()> {
        let relver_str = self.version_to_str(&relver, false);
        let refspec = format!("refs/heads/{}", relver_str);
        self.push(&refspec)
    }

    /// Push the provided 'tagver' release version tag to this repository's
    /// read-write remote.
    ///
    pub fn push_release_tag(self: &Self, tagver: &Version) -> RepositoryResult<()> {
        let tagver_str = self.version_to_str(&tagver, true);
        let refspec = format!("refs/tags/{}", tagver_str);
        self.push(&refspec)
    }

    /// Set a given submodule 'name' head to the provided 'name_spec'. The
    /// function requires the 'is_tag' argument to be provided to build the
    /// correct refspec from 'name_spec'.
    ///
    pub fn set_submodule_head(
        self: &Self,
        name: &String,
        name_spec: &String,
        is_tag: bool,
    ) -> RepositoryResult<PathBuf> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        log::trace!(
            "Set submodule '{}' head to {} '{}'",
            name,
            if is_tag { "tag" } else { "head" },
            name_spec
        );
        let refname = format!(
            "refs/{}/{}",
            if is_tag { "tags" } else { "heads" },
            name_spec
        );
        let path = match git.set_submodule_head(&name, &refname) {
            Ok(p) => {
                log::debug!("Success setting submodule '{}' head to '{}'", name, refname);
                p
            }
            Err(()) => {
                log::error!("Error setting submodule '{}' head to '{}'", name, refname);
                return Err(RepositoryError::SubmoduleHeadUpdateError);
            }
        };

        Ok(path)
    }

    /// Add paths in provided vector to this repository's index, for subsequent commit.
    ///
    pub fn stage_paths(self: &Self, paths: &Vec<PathBuf>) -> RepositoryResult<()> {
        let git = match git::repo::GitRepo::open(&self.path) {
            Ok(r) => r,
            Err(()) => {
                log::error!("Unable to open git repository at '{}'", self.path.display());
                return Err(RepositoryError::UnableToOpenRepositoryError);
            }
        };
        log::debug!(
            "Staging paths: {}",
            paths
                .iter()
                .map(|e| e.to_str().unwrap())
                .collect::<Vec<&str>>()
                .join(", ")
        );
        match git.stage(&paths) {
            Ok(()) => {
                log::debug!("Staged paths!");
            }
            Err(()) => {
                log::error!("Unable to stage paths!");
                return Err(RepositoryError::StagingError);
            }
        };
        Ok(())
    }

    /// Commit a given release version, tagging it with the appropriate version.
    ///
    pub fn commit_release(self: &Self, relver: &Version, tagver: &Version) -> RepositoryResult<()> {
        let relver_str = format!("v{}", relver);
        let commit_msg = if let Some(rc) = &tagver.rc {
            format!("release candidate {} for {}", rc, relver_str)
        } else {
            format!("release {}", relver_str)
        };

        log::debug!("Committing release ver '{}' tag '{}'", relver, tagver);
        match std::process::Command::new("git")
            .args([
                "-C",
                self.path.to_str().unwrap(),
                "commit",
                "--gpg-sign",
                "--signoff",
                "-m",
                commit_msg.as_str(),
            ])
            .status()
        {
            Ok(res) => {
                if !res.success() {
                    log::error!("Unable to commit '{}': {}", tagver, res.code().unwrap());
                    return Err(RepositoryError::UnknownError);
                }
            }
            Err(err) => {
                log::error!("Unable to commit '{}': {}", tagver, err);
                return Err(RepositoryError::UnknownError);
            }
        };

        let tag_name = self.version_to_str(&tagver, true);
        log::debug!("Tag release with '{}'", tag_name);
        match self.tag_release_branch(&relver, &tagver) {
            Ok(_) => {
                log::debug!("Tagged release with '{}'", tag_name);
            }
            Err(err) => {
                log::error!("Error tagging release with '{}': {}", tag_name, err);
                return Err(err);
            }
        };

        Ok(())
    }
}
