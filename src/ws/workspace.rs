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

use super::{config::WSConfig, repository::Repos};

pub struct WSState {}

mod info;
mod sync;

pub struct Workspace {
    path: PathBuf,
    config: WSConfig,
    state: Option<WSState>,
    pub repos: Repos,
}

impl Workspace {
    /// Open an existing workspace at 'path'.
    ///
    pub fn open(path: &PathBuf) -> Result<Workspace, ()> {
        let arcpath = path.join(".arc");
        let cfgpath = arcpath.join("config.json");
        let statepath = arcpath.join("state.json");

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
            state: None,
            repos,
        })
    }

    /// Obtain config directory for this workspace
    pub fn get_config_dir(self: &Self) -> PathBuf {
        self.path.clone().join(".arc")
    }
}
