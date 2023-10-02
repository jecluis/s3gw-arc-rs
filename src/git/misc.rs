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

use super::repo::GitRepo;

// NOTE(joao): this file contains dead code. It's full of auxiliary functions
// that were created for one reason or another, but are not currently used. They
// are being left in the repo because 1) they can act as good examples on how to
// achieve some things, and 2) they may be useful at some point, maybe?

impl GitRepo {
    pub fn _get_default_branch(self: &Self) -> String {
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

    pub fn _get_graph_diff(
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

    pub fn _test_ssh(self: &Self) {
        let mut remote = self.get_remote("rw").unwrap();
        let mut conn = match self.open_remote(&mut remote, git2::Direction::Fetch, true) {
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

    pub fn _find_branch(self: &Self, name: &String, is_remote: &bool) -> Result<git2::Branch, ()> {
        match self.repo.find_branch(
            &name.as_str(),
            match is_remote {
                true => git2::BranchType::Remote,
                false => git2::BranchType::Local,
            },
        ) {
            Ok(b) => Ok(b),
            Err(err) => {
                log::error!(
                    "Unable to find branch {} '{}': {}",
                    match is_remote {
                        true => "remote",
                        false => "local",
                    },
                    name,
                    err
                );
                Err(())
            }
        }
    }

    pub fn _tag_branch(self: &Self, branchname: &String, tagname: &String) -> Result<(), ()> {
        let obj = match self.repo.revparse_single(&branchname) {
            Ok(o) => o,
            Err(err) => {
                log::error!("Unable to find branch '{}': {}", branchname, err);
                return Err(());
            }
        };

        let sig = match self.repo.signature() {
            Ok(s) => s,
            Err(err) => {
                log::error!("Unable to obtain signature: {}", err);
                return Err(());
            }
        };

        let msg = format!("testing tagging for '{}'", tagname);
        let tag = match self
            .repo
            .tag(tagname.as_str(), &obj, &sig, msg.as_str(), false)
        {
            Ok(t) => t,
            Err(err) => {
                log::error!(
                    "Unable to tag branch '{}' (oid: {}) with '{}': {}",
                    branchname,
                    obj.id(),
                    tagname,
                    err
                );
                return Err(());
            }
        };

        log::info!("Tagged branch '{}' with '{}': {}", branchname, tagname, tag);

        Ok(())
    }
}
