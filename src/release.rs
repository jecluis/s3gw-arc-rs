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

use crate::version::Version;
use crate::ws::workspace::Workspace;

pub mod cmds;
mod common;
mod cont;
pub mod errors;
mod list;
mod start;
mod sync;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ReleaseState {
    pub release_version: Version,
}

pub struct Release {
    pub state: Option<ReleaseState>,
    pub confdir: PathBuf,
    pub ws: Workspace,
}

impl Release {
    /// Opens a release in a given workspace, taking ownership of the associated
    /// workspace. A release state may or may not exist.
    ///
    pub fn open(ws: Workspace) -> Result<Release, ()> {
        let configdir = ws.get_config_dir();
        if !configdir.exists() {
            log::error!("Error opening config dir at '{}'", configdir.display());
            return Err(());
        }

        let mut state = Release {
            state: None,
            confdir: configdir.to_path_buf(),
            ws,
        };
        let statefile = configdir.join("release.json");
        if statefile.exists() {
            let f = match std::fs::File::open(&statefile) {
                Ok(v) => v,
                Err(_) => {
                    log::error!("Error opening state file at '{}'", &statefile.display());
                    return Err(());
                }
            };
            state.state = match serde_json::from_reader(f) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Error reading state from '{}': {}", statefile.display(), e);
                    return Err(());
                }
            }
        }
        Ok(state)
    }

    pub fn write(self: &Self) -> Result<(), ()> {
        assert!(self.confdir.exists());

        let state = match &self.state {
            None => {
                log::debug!("No state to write to file!");
                return Ok(());
            }
            Some(v) => v,
        };

        let statefile = self.confdir.join("release.json");
        let f = match std::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&statefile)
        {
            Ok(v) => v,
            Err(e) => {
                log::error!(
                    "Error opening state file at '{}' for writing: {}",
                    &statefile.display(),
                    e
                );
                return Err(());
            }
        };
        match serde_json::to_writer(f, &state) {
            Ok(_) => {
                log::debug!("State written to '{}'", statefile.display());
            }
            Err(e) => {
                log::error!("Error writting state to '{}': {}", &statefile.display(), e);
                return Err(());
            }
        };

        Ok(())
    }

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
    }
}
