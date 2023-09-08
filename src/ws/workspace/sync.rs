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

use crate::ws::repository::Repository;

use super::Workspace;

struct SyncRepo<'a> {
    pub name: String,
    pub update_submodules: bool,
    pub repo: &'a Repository,
}

impl Workspace {
    /// Synchronize the current workspace, showing progress bars for each
    /// individual repository in the workspace.
    ///
    pub fn sync(self: &Self) -> Result<(), ()> {
        let repos: Vec<SyncRepo> = vec![
            SyncRepo {
                name: "s3gw".into(),
                update_submodules: true,
                repo: &self.repos.s3gw,
            },
            SyncRepo {
                name: "ui".into(),
                update_submodules: false,
                repo: &self.repos.ui,
            },
            SyncRepo {
                name: "charts".into(),
                update_submodules: false,
                repo: &self.repos.charts,
            },
            SyncRepo {
                name: "ceph".into(),
                update_submodules: false,
                repo: &self.repos.ceph,
            },
        ];

        for entry in repos {
            match entry.repo.sync(entry.update_submodules) {
                Ok(_) => {}
                Err(_) => {
                    return Err(());
                }
            };
        }

        Ok(())
    }
}
