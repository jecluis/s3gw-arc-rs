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

use crate::infoln;

use super::{
    config::WSConfig,
    repository::{Repos, Repository},
};

struct SyncRepo<'a> {
    #[allow(dead_code)]
    pub name: String,
    pub update_submodules: bool,
    pub repo: &'a Repository,
}

#[derive(Clone)]
pub struct Workspace {
    path: PathBuf,
    #[allow(dead_code)]
    config: WSConfig,
    pub repos: Repos,
}

impl Workspace {
    /// Open an existing workspace at 'path'.
    ///
    pub fn open(path: &PathBuf) -> Result<Workspace, ()> {
        let arcpath = path.join(".arc");
        let cfgpath = arcpath.join("config.json");

        if !arcpath.exists() || !cfgpath.exists() {
            log::error!("Workspace at {} does not exist!", path.display());
            return Err(());
        }

        let cfg = match WSConfig::read(&cfgpath) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open workspace config at {}", cfgpath.display());
                return Err(());
            }
        };

        let repos = match Repos::init(&path, &cfg.user, &cfg.git) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };

        Ok(Workspace {
            path: path.to_path_buf(),
            config: cfg,
            repos,
        })
    }

    /// Obtain config directory for this workspace
    pub fn get_config_dir(self: &Self) -> PathBuf {
        self.path.clone().join(".arc")
    }

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

        infoln!("Synchronize workspace...");
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
