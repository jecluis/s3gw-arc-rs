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

use std::collections::BTreeMap;

use crate::ws::version::Version;

use super::Release;

impl Release {
    pub fn status(self: &Self) {
        log::debug!("Show release status");

        if self.state.is_none() {
            println!("Release not defined");
            return;
        }

        match self.ws.sync() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error synchronizing workspace!");
                return;
            }
        };

        let state = self.state.as_ref().unwrap();
        println!("Release version: {}", state.release_version);

        let release_versions = match Release::get_repo_versions_per_release(&self.ws.repos.s3gw) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to obtain s3gw's releases");
                return;
            }
        };

        let relversion_str = state.release_version.get_release_version_str();
        let relversions = match release_versions.versions_per_release.get(&relversion_str) {
            Some(v) => v,
            None => {
                println!("Release version {} not started.", relversion_str);
                return;
            }
        };

        struct VersionDesc {
            pub version: Version,
            pub rcs: Vec<Version>,
            pub is_complete: bool,
        }

        let mut versions_tree: BTreeMap<u64, VersionDesc> = BTreeMap::new();
        for v in relversions {
            let base_version = v.get_base_version();
            let base_version_id = base_version.get_version_id();
            if !versions_tree.contains_key(&base_version_id) {
                versions_tree.insert(
                    base_version_id,
                    VersionDesc {
                        version: base_version,
                        rcs: vec![],
                        is_complete: false,
                    },
                );
            }
            let desc = versions_tree.get_mut(&base_version_id).unwrap();
            desc.rcs.push(v.clone());

            if v.get_version_id() == base_version_id {
                desc.is_complete = true;
            }
        }

        for desc in versions_tree.values() {
            let avail_ver_str = desc
                .rcs
                .iter()
                .map(|e: &Version| e.get_version_str())
                .collect::<Vec<String>>()
                .join(", ");
            println!(
                "versions for {}: {} ({})",
                desc.version,
                avail_ver_str,
                match desc.is_complete {
                    true => "complete",
                    false => "in progress",
                }
            );
        }
    }
}
