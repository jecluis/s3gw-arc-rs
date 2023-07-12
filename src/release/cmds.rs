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

#[derive(clap::Subcommand)]
pub enum Cmds {
    /// Obtain release information.
    Info,
    /// List releases.
    List,
    /// Initiate a release.
    Init(InitCommand),
}

#[derive(clap::Args)]
pub struct InitCommand {
    #[arg(value_name = "VERSION", short, long)]
    release: Option<String>,
}

pub fn handle_cmds(cmd: &Cmds) {
    let path = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            log::error!("Unable to obtain current directory: {}", e);
            return;
        }
    };
    let ws = match crate::ws::init::open(&path) {
        Ok(v) => v,
        Err(_) => {
            log::error!("Unable to open workspace at {}", path.display());
            return;
        }
    };

    match cmd {
        Cmds::List => {
            log::debug!("List existing releases");
            crate::release::Release::list(&ws);
            return;
        }
        Cmds::Init(cmd) => {
            log::debug!("Init release");
            match crate::release::Release::init(ws, &cmd.release) {
                Ok(release) => {
                    println!("Release {} init'ed.", release.get_version());
                }
                Err(e) => {
                    log::error!("Error init'ing release: {:?}", e);
                }
            };
            return;
        }
        _ => {}
    };

    let release = match crate::release::Release::open(ws) {
        Ok(r) => r,
        Err(_) => {
            log::error!("Unable to open workspace release config!");
            return;
        }
    };

    match cmd {
        Cmds::Info => {
            log::debug!("Obtain workspace release info");
        }
        Cmds::List => {
            log::error!("Should not have reached here!");
            return;
        }
        Cmds::Init(_) => {
            log::error!("Should not have reached here!");
            return;
        }
    };
}