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

#[derive(clap::Subcommand)]
pub enum Cmds {
    Init(InitCommand),
    Info,
}

#[derive(clap::Args)]
pub struct InitCommand {
    /// Workspace Path
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
}

/// Handles workspace-related commands.
///
pub fn handle_cmds(cmd: &Cmds) {
    match cmd {
        Cmds::Init(init) => {
            log::info!("Create workspace at {}", init.path.display());
            match super::init::init(&init.path) {
                Ok(_) => {
                    log::info!("Success!");
                }
                Err(_) => {
                    log::error!("Error!");
                }
            };
            return;
        }
        _ => {}
    }

    let path = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            log::error!("Unable to obtain current directory: {}", e);
            return;
        }
    };
    let ws = match super::init::open(&path) {
        Ok(v) => v,
        Err(_) => {
            log::error!("Unable to open workspace at {}", path.display());
            return;
        }
    };

    match cmd {
        Cmds::Init(_) => {
            log::error!("Should never reach this point!");
            return;
        }
        Cmds::Info => {
            ws.info();
        }
    };
}