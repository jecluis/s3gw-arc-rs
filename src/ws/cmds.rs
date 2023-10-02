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

use crate::{boomln, errorln, infoln, successln};

#[derive(clap::Subcommand)]
pub enum Cmds {
    Init(InitCommand),
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
            infoln!("Create workspace at {}", init.path.display());
            match super::init::init(&init.path) {
                Ok(_) => {
                    successln!("Success!");
                }
                Err(_) => {
                    boomln!("Error!");
                }
            };
            return;
        }
        #[allow(unreachable_patterns)]
        _ => {}
    }

    let path = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            boomln!("Unable to obtain current directory: {}", e);
            return;
        }
    };
    let _ws = match super::init::open(&path) {
        Ok(v) => v,
        Err(err) => {
            errorln!("Unable to open workspace at {}: {}", path.display(), err);
            return;
        }
    };

    match cmd {
        Cmds::Init(_) => {
            boomln!("Should never reach this point!");
            return;
        }
    };
}
