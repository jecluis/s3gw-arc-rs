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

use crate::common::UpdateProgress;

pub struct GitRepo {
    path: PathBuf,
    pub(crate) repo: git2::Repository,
}

impl GitRepo {
    pub fn get_git_repo(self: &Self) -> &git2::Repository {
        &self.repo
    }

    /// Clone a repository into 'path', using the upstream remotes 'ro' and
    /// 'rw'. 'ro' refers to a read-only URI, and 'rw' as a read-write URI.
    ///
    pub fn clone(
        path: &PathBuf,
        ro: &String,
        rw: &String,
        progress_desc: &String,
    ) -> Result<GitRepo, ()> {
        if path.exists() {
            log::error!("Directory exists at {}, can't clone.", path.display());
            return Err(());
        }

        let mut progress = crate::common::RepoSyncProgress::new(progress_desc);
        let cb = |p: git2::Progress| {
            progress.handle_values(
                "clone",
                p.received_objects() as u64,
                p.indexed_objects() as u64,
                p.total_objects() as u64,
                p.indexed_deltas() as u64,
                p.total_deltas() as u64,
            );
        };
        let repo = match GitRepo::do_clone(&path, &ro, &rw, cb) {
            Err(()) => {
                progress.finish_with_error();
                return Err(());
            }
            Ok(r) => {
                progress.finish();
                r
            }
        };

        Ok(GitRepo {
            path: path.to_path_buf(),
            repo,
        })
    }

    /// Performs the actual clone.
    ///
    fn do_clone<F>(
        path: &PathBuf,
        ro: &String,
        rw: &String,
        mut cb: F,
    ) -> Result<git2::Repository, ()>
    where
        F: FnMut(git2::Progress),
    {
        let mut builder = git2::build::RepoBuilder::new();
        let mut cbs = git2::RemoteCallbacks::new();
        cbs.transfer_progress(|progress: git2::Progress| {
            cb(progress);
            true
        });
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(cbs);
        let repo = match builder.fetch_options(opts).clone(&ro, &path) {
            Err(err) => {
                log::error!("Unable to clone repository to {}: {}", path.display(), err);
                return Err(());
            }
            Ok(r) => {
                r.remote_rename("origin", "ro")
                    .expect("error renaming origin");
                r.remote("rw", rw.as_str()).expect("error adding rw remote");
                r
            }
        };

        Ok(repo)
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

        Ok(GitRepo {
            path: path.to_path_buf(),
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

    /// Obtain a given remote by name.
    ///
    pub(crate) fn get_remote(self: &Self, name: &str) -> Result<git2::Remote, ()> {
        match self.repo.find_remote(name) {
            Ok(r) => Ok(r),
            Err(e) => {
                log::error!("Unable to find remote '{}': {}", name, e);
                return Err(());
            }
        }
    }

    /// Open a connection for the provided remote. If 'with_auth' is true, then
    /// the connection will be authenticated using the user's ssh key agent.
    ///
    pub(crate) fn open_remote<'a, 'b>(
        self: &'a Self,
        remote: &'b mut git2::Remote<'a>,
        direction: git2::Direction,
        with_auth: bool,
    ) -> Result<git2::RemoteConnection<'a, 'b, '_>, ()> {
        let cbs: Option<git2::RemoteCallbacks> = if with_auth {
            let mut cbs = git2::RemoteCallbacks::new();
            cbs.credentials(|url, user, allowed_types| {
                let username = user.unwrap();
                log::trace!(
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

    /// Update a single remote for this repository, by name.
    ///
    fn do_remote_update_single(self: &Self, name: &str, auth: bool) -> Result<(), ()> {
        let mut remote = self.get_remote(name).unwrap();
        let mut conn = match self.open_remote(&mut remote, git2::Direction::Fetch, auth) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open remote '{}'", name);
                return Err(());
            }
        };
        let mut opts = git2::FetchOptions::new();
        opts.download_tags(git2::AutotagOption::All);

        let remote = conn.remote();
        log::debug!("Updating remote '{}'", name);
        let x: [&str; 0] = [];
        match remote.fetch(&x, Some(&mut opts), None) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Unable to update remote '{}': {}", name, e);
                return Err(());
            }
        };
        log::debug!("Remote '{}' updated", name);
        Ok(())
    }

    /// Update default remotes. This means 'ro' and 'rw'.
    ///
    pub fn remote_update(self: &Self, progress_desc: &String) -> Result<(), ()> {
        let remotes = vec![("ro", false), ("rw", true)];
        self.remote_update_vec(&progress_desc, &remotes)
    }

    /// Update remotes specified by the provided vector.
    ///
    pub fn remote_update_vec(
        self: &Self,
        progress_desc: &String,
        remotes: &Vec<(&str, bool)>,
    ) -> Result<(), ()> {
        let progress = UpdateProgress::new(&progress_desc);
        progress.start();

        for (remote, auth) in remotes {
            progress.set_message(&String::from(*remote));

            match self.do_remote_update_single(remote, *auth) {
                Ok(()) => {}
                Err(()) => {
                    progress.finish_with_error();
                    return Err(());
                }
            };
        }
        progress.finish();

        Ok(())
    }

    /// Update this repository's submodules, if any exist. This may mean
    /// downloading the submodule repository if it hasn't been done so yet.
    /// This function outputs progress bars for the operation.
    ///
    pub fn submodules_update(self: &Self) -> Result<(), ()> {
        let mut submodules = match self.repo.submodules() {
            Ok(v) => v,
            Err(err) => {
                log::error!("Error obtaining repository's submodules: {}", err);
                return Err(());
            }
        };

        for sm in &mut submodules {
            let sm_name = match sm.name() {
                None => "N/A".into(),
                Some(n) => {
                    let p = n.split("/").last().unwrap_or("N/A");
                    String::from(p)
                }
            };
            let mut progress = crate::common::RepoSyncProgress::new(&format!("sub {}", sm_name));
            let cb = |p: git2::Progress| {
                progress.handle_values(
                    "submodule update",
                    p.received_objects() as u64,
                    p.indexed_objects() as u64,
                    p.total_objects() as u64,
                    p.indexed_deltas() as u64,
                    p.total_deltas() as u64,
                );
            };
            log::debug!("Update submodule {}", sm_name);
            match self.do_submodule_update(sm, cb) {
                Ok(()) => {
                    progress.finish();
                }
                Err(()) => {
                    progress.finish_with_error();
                    log::error!("Error updating submodule {}", sm_name);
                    return Err(());
                }
            };
        }
        Ok(())
    }

    /// Helper function. Performs the actual submodule update, returning
    /// progress via the provided callback function 'cb'.
    /// It will first attempt to update the submodule, cloning it if necessary,
    /// and then will attempt to synchronize the submodule's URLs with its remote.
    ///
    fn do_submodule_update<F>(self: &Self, sm: &mut git2::Submodule, mut cb: F) -> Result<(), ()>
    where
        F: FnMut(git2::Progress),
    {
        let mut opts = git2::SubmoduleUpdateOptions::new();
        let mut fetch_opts = git2::FetchOptions::new();
        let mut cbs = git2::RemoteCallbacks::new();
        cbs.transfer_progress(|progress: git2::Progress| {
            cb(progress);
            true
        });
        fetch_opts.remote_callbacks(cbs);
        opts.fetch(fetch_opts);

        match sm.update(true, Some(&mut opts)) {
            Ok(()) => {
                log::debug!("Updated submodule");
            }
            Err(err) => {
                log::error!("Unable to update submodule: {}", err);
                return Err(());
            }
        };
        match sm.sync() {
            Ok(()) => {
                log::debug!("Sync'ed submodule");
            }
            Err(err) => {
                log::error!("Unable to sync submodule: {}", err);
                return Err(());
            }
        };
        Ok(())
    }

    /// Obtain a vector containing all references associated with this
    /// repository. References are obtained from the read-only 'ro' remote.
    ///
    pub fn get_refs(self: &Self) -> Result<super::refs::GitRefMap, ()> {
        let mut remote = self.get_remote("ro").unwrap();
        let mut conn = match self.open_remote(&mut remote, git2::Direction::Fetch, false) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open remote to obtain refs!");
                return Err(());
            }
        };
        let remote = conn.remote();
        let refs = match super::refs::get_refs_from(&remote, &self.repo) {
            Ok(v) => v,
            Err(()) => {
                log::error!("Unable to obtain references!");
                return Err(());
            }
        };

        Ok(refs)
    }

    /// Create a branch from this repository's default branch.
    ///
    pub fn branch_from_default(self: &Self, dst: &String) -> Result<(), ()> {
        let head_ref = self.repo.find_reference("refs/remotes/ro/HEAD").unwrap();
        let head_name = head_ref.symbolic_target().unwrap();
        let head_commit = head_ref.peel_to_commit().unwrap();

        log::debug!(
            "branch to: {}, head: name = {}, commit: {}",
            dst,
            head_name,
            head_commit.id()
        );

        match self.repo.branch(&dst, &head_commit, false) {
            Ok(_) => Ok(()),
            Err(_) => {
                log::error!(
                    "Unable to branch off '{}' ({}) to '{}'",
                    head_name,
                    head_commit.id(),
                    dst
                );
                return Err(());
            }
        }
    }

    /// Checks out the provided branch 'name'.
    ///
    pub fn checkout_branch(self: &Self, name: &String) -> Result<(), ()> {
        let refname = format!("refs/heads/{}", name);
        match self.repo.set_head(&refname) {
            Ok(()) => {}
            Err(err) => {
                log::error!("Error setting repository's head to '{}': {}", name, err);
                return Err(());
            }
        };
        match self
            .repo
            .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        {
            Ok(()) => {}
            Err(err) => {
                log::error!("Error checking out repository's head: {}", err);
                return Err(());
            }
        };

        Ok(())
    }

    /// Obtains an object for the provided 'refspec', if it exists.
    ///
    pub fn get_oid_by_refspec(self: &Self, refspec: &String) -> Result<git2::Object, ()> {
        match self.repo.revparse_single(&refspec) {
            Ok(o) => Ok(o),
            Err(err) => {
                log::error!("Unable to find oid for '{}': {}", refspec, err);
                return Err(());
            }
        }
    }

    /// Pushes the provided 'refspec' to this repository's read-write 'rw' remote.
    ///
    // TODO(joao): make it output progress bars.
    pub fn push(self: &Self, refspec: &String) -> Result<(), ()> {
        let mut remote = match self.get_remote("rw") {
            Ok(r) => r,
            Err(()) => {
                log::error!("Error obtaining 'rw' remote to push refspec '{}'", refspec);
                return Err(());
            }
        };
        let mut conn = match self.open_remote(&mut remote, git2::Direction::Push, true) {
            Ok(c) => c,
            Err(()) => {
                log::error!(
                    "Error opening remote 'rw' connection to push refspec '{}'",
                    refspec
                );
                return Err(());
            }
        };

        let remote = conn.remote();
        match remote.push(&[refspec], None) {
            Ok(()) => {
                log::trace!("Pushed refspec '{}'", refspec);
            }
            Err(err) => {
                log::error!("Unable to push refspec '{}' to rw remote: {}", refspec, err);
                return Err(());
            }
        };

        Ok(())
    }

    /// Fetch a given refspec, branching the resulting FETCH_HEAD into a branch
    /// with the provided 'dst_branch_name' name.
    ///
    pub fn fetch(self: &Self, refspec: &String, dst_branch_name: &String) -> Result<(), ()> {
        let mut remote = match self.get_remote("ro") {
            Ok(r) => r,
            Err(()) => {
                log::error!("Error obtaining 'rw' remote to fetch refspec '{}'", refspec);
                return Err(());
            }
        };
        match remote.fetch(&[refspec], None, None) {
            Ok(()) => {
                log::debug!("Fetched refspec '{}'", refspec);
            }
            Err(err) => {
                log::error!("Error fetching refspec '{}': {}", refspec, err);
                return Err(());
            }
        };

        let fetch_head = self.repo.find_reference("FETCH_HEAD").unwrap();
        let commit = self
            .repo
            .reference_to_annotated_commit(&fetch_head)
            .unwrap();
        match self
            .repo
            .branch_from_annotated_commit(&dst_branch_name, &commit, true)
        {
            Ok(_) => {
                log::debug!(
                    "Successfully branched from FETCH_HEAD to '{}'",
                    dst_branch_name
                );
            }
            Err(err) => {
                log::error!(
                    "Error branching from FETCH_HEAD to '{}': {}",
                    dst_branch_name,
                    err
                );
                return Err(());
            }
        };

        Ok(())
    }

    /// Set a given submodule 'name's HEAD to the provided 'refname'.
    ///
    pub fn set_submodule_head(self: &Self, name: &String, refname: &String) -> Result<PathBuf, ()> {
        let submodule = match self.repo.find_submodule(&name) {
            Ok(s) => s,
            Err(err) => {
                log::error!("Unable to find submodule '{}': {}", name, err);
                return Err(());
            }
        };

        let submodule_path = submodule.path();
        let repo_path = self.path.join(submodule_path);
        if !repo_path.exists() {
            log::error!(
                "Submodule '{}' path at '{}' does not exist!",
                name,
                repo_path.display()
            );
            return Err(());
        }

        let repo = match GitRepo::open(&repo_path.to_path_buf()) {
            Ok(r) => r,
            Err(()) => {
                log::error!(
                    "Unable to open git repository at '{}'!",
                    repo_path.display()
                );
                return Err(());
            }
        };

        match repo.remote_update_vec(&format!("sub {}", name), &vec![("origin", false)]) {
            Ok(()) => {}
            Err(()) => {
                log::error!("Unable to update 'origin' for submodule '{}'", name);
                return Err(());
            }
        };

        let git = repo.get_git_repo();
        match git.set_head(&refname) {
            Ok(()) => {
                log::debug!("Set submodule's head to '{}'", refname);
            }
            Err(err) => {
                log::error!("Error setting submodule's head to '{}': {}", refname, err);
                return Err(());
            }
        };
        match git.checkout_head(Some(git2::build::CheckoutBuilder::default().force())) {
            Ok(()) => {}
            Err(err) => {
                log::error!(
                    "Error checking out object oid '{}' in submodule '{}': {}",
                    refname,
                    name,
                    err
                );
                return Err(());
            }
        };

        Ok(submodule_path.to_path_buf())
    }

    /// Stage the paths provided in a vector 'paths', by adding them to the
    /// index. This does not perform a commit.
    ///
    pub fn stage(self: &Self, paths: &Vec<PathBuf>) -> Result<(), ()> {
        let mut index = match self.repo.index() {
            Ok(idx) => idx,
            Err(err) => {
                log::error!("Unable to obtain repository's index: {}", err);
                return Err(());
            }
        };
        for path in paths {
            match index.add_path(path) {
                Ok(()) => {
                    log::trace!("Added '{}' to index", path.display());
                }
                Err(err) => {
                    log::error!("Error adding '{}' to index: {}", path.display(), err);
                    return Err(());
                }
            };
        }
        match index.write() {
            Ok(()) => {
                log::debug!("Wrote index to disk");
            }
            Err(err) => {
                log::error!("Error writing index to disk: {}", err);
                return Err(());
            }
        };

        Ok(())
    }
}
