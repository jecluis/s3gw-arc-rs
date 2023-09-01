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

pub struct GitRepo {
    path: PathBuf,
    ro: String,
    rw: String,
    repo: git2::Repository,
}

impl GitRepo {
    /// Clone a repository into 'path', using the upstream remotes 'ro' and
    /// 'rw'. 'ro' refers to a read-only URI, and 'rw' as a read-write URI.
    /// Operation progress will be tracked by 'progress_cb'.
    ///
    pub fn clone<F>(
        path: &PathBuf,
        ro: &String,
        rw: &String,
        mut progress_cb: F,
    ) -> Result<GitRepo, ()>
    where
        F: FnMut(u64, u64),
    {
        if path.exists() {
            log::error!("Directory exists at {}, can't clone.", path.display());
            return Err(());
        }
        let mut builder = git2::build::RepoBuilder::new();
        let mut cbs = git2::RemoteCallbacks::new();
        cbs.transfer_progress(|progress: git2::Progress| {
            progress_cb(
                progress.received_objects() as u64,
                progress.total_objects() as u64,
            );
            true
        });
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(cbs);
        let repo = match builder.fetch_options(opts).clone(&ro, &path) {
            Err(e) => {
                log::error!("Unable to clone repository to {}: {}", path.display(), e);
                return Err(());
            }
            Ok(r) => {
                r.remote_rename("origin", "ro")
                    .expect("error renaming origin");
                r.remote("rw", rw.as_str()).expect("error adding rw remote");
                r
            }
        };

        Ok(GitRepo {
            path: path.to_path_buf(),
            ro: ro.clone(),
            rw: rw.clone(),
            repo,
        })
    }

    /// Open an existing git repository at 'path'.
    ///
    pub fn open(path: &PathBuf) -> Result<GitRepo, ()> {
        let repo = match git2::Repository::open(path) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Error opening repository at {}: {}", path.display(), e);
                return Err(());
            }
        };

        fn get_remote_url(r: &git2::Remote) -> String {
            String::from(r.url().unwrap())
        }

        let ro = match repo.find_remote("ro") {
            Ok(v) => get_remote_url(&v),
            Err(e) => {
                log::error!(
                    "Unable to obtain read-only remote for {}: {}",
                    path.display(),
                    e
                );
                return Err(());
            }
        };
        let rw = match repo.find_remote("rw") {
            Ok(v) => get_remote_url(&v),
            Err(e) => {
                log::error!(
                    "Unable to obtain read-write remote for {}: {}",
                    path.display(),
                    e
                );
                return Err(());
            }
        };

        Ok(GitRepo {
            path: path.to_path_buf(),
            ro,
            rw,
            repo,
        })
    }

    /// set user name.
    pub fn set_user_name(self: &Self, name: &str) -> &Self {
        self.repo
            .config()
            .unwrap()
            .set_str("user.name", name)
            .unwrap();
        self
    }

    /// set user email.
    pub fn set_user_email(self: &Self, email: &str) -> &Self {
        self.repo
            .config()
            .unwrap()
            .set_str("user.email", email)
            .unwrap();
        self
    }

    /// set signing key and force commit gpg sign.
    pub fn set_signing_key(self: &Self, key: &str) -> &Self {
        let mut cfg = self.repo.config().unwrap();
        cfg.set_str("user.signingKey", key).unwrap();
        cfg.set_bool("commit.gpgSign", true).unwrap();
        self
    }

    // pub fn _get_refs(self: &Self) -> Result<(), ()> {
    //     let branches = match self.repo.branches(None) {
    //         Ok(v) => v,
    //         Err(e) => {
    //             log::error!(
    //                 "Error obtaining repository {} branches: {}",
    //                 self.path.display(),
    //                 e
    //             );
    //             return Err(());
    //         }
    //     };

    //     for entry in branches {
    //         if let Err(e) = entry {
    //             log::error!("Error listing branches: {}", e);
    //             return Err(());
    //         }
    //         let (branch, branch_type) = entry.unwrap();
    //         log::debug!(
    //             "branch '{}' type '{}'",
    //             branch.name().unwrap().unwrap(),
    //             match branch_type {
    //                 git2::BranchType::Local => "local",
    //                 git2::BranchType::Remote => "remote",
    //             }
    //         );
    //     }

    //     log::debug!("Obtaining tags...");
    //     match self.repo.tag_names(None /*Some("s3gw-*")*/) {
    //         Ok(t) => {
    //             for tag in t.iter() {
    //                 log::debug!("tag found: {}", tag.unwrap());
    //             }
    //         }
    //         Err(e) => {
    //             log::error!("Error obtaining tags: {}", e);
    //         }
    //     };

    //     let mut ro = self.repo.find_remote("ro").unwrap();
    //     ro.connect(git2::Direction::Fetch)
    //         .expect("Unable to connect to remote");

    //     log::debug!("List repository...");
    //     let rols = ro.list().unwrap();
    //     for head in rols {
    //         let tgt = head.symref_target().unwrap_or("N/A");
    //         let oid = head.oid();
    //         let mut kind = "N/A";
    //         if let Ok(obj) = self.repo.find_object(oid, None) {
    //             kind = match obj
    //                 .kind()
    //                 .expect(format!("Unable to obtain kind for oid {}", oid).as_str())
    //             {
    //                 git2::ObjectType::Any => "any",
    //                 git2::ObjectType::Blob => "blob",
    //                 git2::ObjectType::Commit => "commit",
    //                 git2::ObjectType::Tag => "tag",
    //                 git2::ObjectType::Tree => "tree",
    //             };
    //         }

    //         log::debug!(
    //             "head: {}, local: {}, target: {}, oid: {}, kind: {}",
    //             head.name(),
    //             head.is_local(),
    //             tgt,
    //             oid,
    //             kind
    //         );
    //     }

    //     log::debug!("default branch: {}", self.get_default_branch());

    //     let branches_and_tags = GitRefs::from_remote(&ro).unwrap();
    //     for entry in branches_and_tags.branches {
    //         log::debug!("branch: {}, oid: {}", entry.name, entry.oid);
    //     }
    //     for entry in branches_and_tags.tags {
    //         log::debug!("tag: {}, oid: {}", entry.name, entry.oid);
    //     }

    //     self._remote_update("ro").expect("Unable to update remote");
    //     let v0180rc1_oid = git2::Oid::from_str("dc657a48600c7a87084252481740463e40faedff")
    //         .expect("Unable to get oid for v0.18.0-rc1");
    //     let v0180rc2_oid = git2::Oid::from_str("18f99637b7c10dfca7575244e2d649dd45610f04")
    //         .expect("Unable to get oid for v0.18.0-rc2");
    //     let (ahead, behind) = self
    //         .get_graph_diff(v0180rc1_oid, v0180rc2_oid)
    //         .expect("Error getting graph diff");
    //     log::debug!("ahead: {}, behind: {}", ahead, behind);

    //     log::debug!("List remote refspecs...");
    //     for refspec in ro.refspecs() {
    //         log::debug!(
    //             "src: {}, dst: {}",
    //             refspec.src().unwrap(),
    //             refspec.dst().unwrap()
    //         );
    //     }

    //     log::debug!("List fetched refspecs...");
    //     let refspecs = ro.fetch_refspecs().expect("Unable to obtain refspecs!");
    //     for refspec in refspecs.iter() {
    //         log::debug!("{}", refspec.unwrap());
    //     }

    //     log::debug!("Obtaining references...");
    //     let mut default_branch: Option<String> = None;
    //     for entry in self.repo.references().unwrap() {
    //         let refentry = entry.unwrap();
    //         let refname = refentry.name().unwrap();
    //         if refentry.is_branch() {
    //             log::debug!("ref branch: {}", refname);
    //         } else if refentry.is_tag() {
    //             log::debug!("ref tag: {}", refname);
    //         } else if refentry.is_remote() {
    //             log::debug!("ref remote: {}", refname);
    //             if refname == "refs/remotes/ro/HEAD" {
    //                 if let Some(tgt) = refentry.symbolic_target() {
    //                     default_branch = Some(String::from(tgt));
    //                     break;
    //                 }
    //             }
    //         } else {
    //             log::debug!("something else: {}", refname);
    //         }
    //         if let Some(tgt) = refentry.symbolic_target() {
    //             log::debug!("  symbolic target: {}", tgt);
    //         }
    //     }
    //     if let Some(def) = default_branch {
    //         let oid = self.repo.refname_to_id(def.as_str()).unwrap();
    //         log::debug!("Default branch: {} ({})", def, oid);
    //     }

    //     Ok(())
    // }

    pub fn get_default_branch(self: &Self) -> String {
        let head_ref = self
            .repo
            .find_reference("refs/remotes/ro/HEAD")
            .expect("Unable to find reference");
        let tgt = head_ref
            .symbolic_target()
            .expect("Unable to get symbolic target for head reference");
        let branch = tgt
            .strip_prefix("refs/remotes/ro/")
            .expect("Unable to obtain branch name from target string!");
        String::from(branch)
    }

    pub fn get_graph_diff(
        self: &Self,
        local: git2::Oid,
        upstream: git2::Oid,
    ) -> Result<(usize, usize), ()> {
        match self.repo.graph_ahead_behind(local, upstream) {
            Ok(v) => Ok(v),
            Err(e) => {
                log::error!("Unable to obtain graph diff: {}", e);
                Err(())
            }
        }
    }

    fn _get_remote(self: &Self, name: &str) -> Result<git2::Remote, ()> {
        match self.repo.find_remote(name) {
            Ok(r) => Ok(r),
            Err(e) => {
                log::error!("Unable to find remote '{}': {}", name, e);
                return Err(());
            }
        }
    }

    fn _open_remote<'a, 'b>(
        self: &'a Self,
        remote: &'b mut git2::Remote<'a>,
        direction: git2::Direction,
        with_auth: bool,
    ) -> Result<git2::RemoteConnection<'a, 'b, '_>, ()> {
        let cbs: Option<git2::RemoteCallbacks> = if with_auth {
            let mut cbs = git2::RemoteCallbacks::new();
            cbs.credentials(|url, user, allowed_types| {
                let username = user.unwrap();
                log::debug!(
                    "auth url: {}, username: {}, allowed_types: {:?}",
                    url,
                    username,
                    allowed_types
                );
                git2::Cred::ssh_key_from_agent(username)
            });
            Some(cbs)
        } else {
            None
        };

        let conn = match remote.connect_auth(direction, cbs, None) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Unable to connect to remote: {}", e);
                return Err(());
            }
        };

        Ok(conn)
    }

    fn _remote_update(self: &Self, name: &str, auth: bool) -> Result<(), ()> {
        let mut remote = self._get_remote(name).unwrap();
        let mut conn = match self._open_remote(&mut remote, git2::Direction::Fetch, auth) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open remote '{}'", name);
                return Err(());
            }
        };
        let remote = conn.remote();
        log::info!("Updating remote '{}'", name);
        let x: [&str; 0] = [];
        match remote.fetch(&x, None, None) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Unable to update remote '{}': {}", name, e);
                return Err(());
            }
        };
        log::info!("Remote '{}' updated", name);
        Ok(())
    }

    pub fn remote_update(self: &Self) -> Result<(), ()> {
        match self._remote_update("ro", false) {
            Ok(()) => {}
            Err(()) => {
                return Err(());
            }
        };
        self._remote_update("rw", true)
    }

    pub fn get_refs(self: &Self) -> Result<super::refs::GitRefs, ()> {
        let mut remote = self._get_remote("ro").unwrap();
        let mut conn = match self._open_remote(&mut remote, git2::Direction::Fetch, false) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open remote to obtain refs!");
                return Err(());
            }
        };
        let remote = conn.remote();
        let refs = match super::refs::GitRefs::from_remote(&remote) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to obtain refs from remote!");
                return Err(());
            }
        };

        Ok(refs)
    }

    pub fn test_ssh(self: &Self) {
        let mut remote = self._get_remote("rw").unwrap();
        let mut conn = match self._open_remote(&mut remote, git2::Direction::Fetch, true) {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to open remote to test ssh!");
                return;
            }
        };
        let remote = conn.remote();
        let x: [&str; 0] = [];
        match remote.fetch(&x, None, None) {
            Ok(_) => {
                log::debug!("Fetched!");
            }
            Err(e) => {
                log::error!("Error fetching: {}", e);
            }
        };

        log::debug!(
            "defaul branch: {}",
            remote.default_branch().unwrap().as_str().unwrap()
        );
    }
}
