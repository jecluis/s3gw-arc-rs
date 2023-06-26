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
}
