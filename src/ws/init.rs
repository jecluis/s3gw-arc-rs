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

use crate::ws::errors::WorkspaceError;

use super::{config::WSConfig, errors::WorkspaceResult, prompt::init_prompt, workspace::Workspace};

/// Create and initiate a new workspace at 'path'.
pub fn init(path: &PathBuf) -> WorkspaceResult<Workspace> {
    let arcpath = path.join(".arc");
    let cfgpath = arcpath.join("config.json");

    if cfgpath.exists() {
        log::error!("Workspace at {} already exists.", path.display());
        return Err(WorkspaceError::AlreadyExistsError);
    } else if !path.exists() || !arcpath.exists() || !cfgpath.exists() {
        match create_workspace(path) {
            Ok(()) => {}
            Err(err) => {
                log::error!("Unable to create workspace at {}: {}", path.display(), err);
                return Err(WorkspaceError::CreationError);
            }
        };
    }

    let ws = match Workspace::open(path) {
        Ok(v) => v,
        Err(err) => {
            log::error!("Error opening workspace at {}: {}", path.display(), err);
            return Err(err);
        }
    };

    match ws.sync() {
        Ok(_) => {}
        Err(()) => {
            log::error!("Error synchronizing workspace at {}", path.display());
            return Err(WorkspaceError::SyncError);
        }
    };

    Ok(ws)
}

/// Open an existing workspace at 'path'.
pub fn open(path: &PathBuf) -> WorkspaceResult<Workspace> {
    match Workspace::open(path) {
        Ok(ws) => Ok(ws),
        Err(err) => {
            log::error!("Error opening workspace at {}: {}", path.display(), err);
            return Err(err);
        }
    }
}

/// Creates a new workspace, obtaining information required from the user (via
/// prompts), and writes a workspace config file.
fn create_workspace(path: &PathBuf) -> WorkspaceResult<()> {
    let arcpath = path.join(".arc");
    if !arcpath.exists() {
        std::fs::create_dir_all(&arcpath).expect("Unable to create directories");
    }

    assert!(arcpath.is_dir());
    let cfgpath = arcpath.join("config.json");
    assert!(!cfgpath.exists());

    let cfg = match init_prompt(&WSConfig::default()) {
        Ok(v) => v,
        Err(err) => {
            log::error!("Unable to generate workspace config: {}", err);
            return Err(WorkspaceError::ConfigError);
        }
    };
    match cfg.write(&cfgpath) {
        Ok(_) => {}
        Err(_) => {
            log::error!("Unable to write workspace config at {}", cfgpath.display());
            return Err(WorkspaceError::ConfigError);
        }
    };
    log::debug!("Wrote workspace config at {}", cfgpath.display());

    Ok(())
}
