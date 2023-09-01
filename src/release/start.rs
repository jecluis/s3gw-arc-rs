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

use crate::ws::{repository::Repository, version::Version};

use super::Release;

impl Release {
    pub fn start(self: &Self, version: &Version) -> Result<(), ()> {
        // 1. sync rw repos to force authorized connect
        // 2. check all repos for existing versions
        // 2.1. make sure this version has not been started in any of the
        // existing repositories.
        // 3. start release procedures.

        match self.ws.sync() {
            Ok(()) => {}
            Err(()) => {
                log::error!("Unable to synchronize workspace repositories!");
                return Err(());
            }
        };

        let min_id = version.min().get_version_id();
        let max_id = version.max().get_version_id();

        println!(
            "v: {}, id: {}, min: {}, max: {}",
            version,
            version.get_version_id(),
            min_id,
            max_id
        );

        let version_tree = self.ws.repos.s3gw.get_versions().unwrap();
        let mut avail = version_tree.range((
            std::ops::Bound::Included(min_id),
            std::ops::Bound::Included(max_id),
        ));
        if avail.any(|(_, ver)| ver == version) {
            println!("version {} has already been released.", version);
            return Err(());
        }

        // NOTE(joao): we should check whether there is a started release across
        // the repositories. This can be done by checking for rc versions on
        // every repository. For now we will ignore this bit.

        if avail.count() > 0 {
            println!("release version {} has already been started.", version);
            return Err(());
        }

        log::info!("start releasing version {}", version);

        self.create_release(&version);

        let version_tree = self.ws.repos.s3gw.get_version_tree().unwrap();
        for base_version in version_tree.values() {
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

        self.ws.repos.s3gw.test_ssh();
        Ok(())
    }

    fn create_release(self: &Self, version: &Version) -> Result<(), ()> {
        // check whether we need to cut branches for each repository
        self.maybe_cut_branches(&version);
        Ok(())
    }

    fn maybe_cut_branches(self: &Self, version: &Version) -> Result<(), ()> {
        let repos = self.ws.repos.as_list();
        let base_version = version.get_base_version();
        let base_version_id = base_version.get_version_id();

        let mut repos_to_cut: Vec<&Repository> = vec![];
        for repo in repos {
            let branches = match repo.get_release_branches() {
                Ok(v) => v,
                Err(()) => {
                    log::error!("unable to obtain branches for release");
                    return Err(());
                }
            };
            if !branches.contains_key(&base_version_id) {
                repos_to_cut.push(repo);
            }
        }

        if repos_to_cut.len() == 0 {
            return Ok(());
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
                return Err(());
            }
            Err(e) => {
                log::error!("Error prompting user: {}", e);
                return Err(());
            }
        };
        Ok(())
    }
}
