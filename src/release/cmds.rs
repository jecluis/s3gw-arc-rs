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

use crate::{boomln, errorln, infoln, successln, version::Version, warnln};

#[derive(clap::Subcommand)]
pub enum Cmds {
    /// Obtain release information.
    Info,
    /// List releases.
    List,
    /// Release status.
    Status,
    /// Start a new release process.
    Start(StartCommand),
    /// Continue the release process.
    Continue(ContinueCommand),
}

#[derive(clap::Args)]
pub struct StartCommand {
    /// Version to start a new release process for (e.g., 0.17.1)
    #[arg(value_name = "VERSION")]
    version: String,

    /// Release notes
    #[arg(value_name = "FILE", short, long)]
    notes: PathBuf,
}

#[derive(clap::Args)]
pub struct ContinueCommand {
    /// Release notes
    #[arg(value_name = "FILE", short, long)]
    notes: PathBuf,

    /// Release version to continue (e.g., v0.17.1)
    #[arg(value_name = "VERSION", short, long)]
    version: Option<String>,
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
            match crate::release::list::list(&ws) {
                Ok(()) => {}
                Err(()) => {
                    boomln!("Unable to list releases!");
                }
            };
            return;
        }
        _ => {}
    };

    let mut release = match crate::release::Release::open(ws) {
        Ok(r) => r,
        Err(_) => {
            boomln!("Unable to open workspace release config!");
            return;
        }
    };

    match cmd {
        Cmds::Info => {
            infoln!("Obtain workspace release info");
        }
        Cmds::Status => {
            log::debug!("Obtain release status");
            release.status();
        }
        Cmds::Start(start_cmd) => {
            infoln!(format!(
                "Start a new release process for version {}",
                start_cmd.version
            ));
            let version = match crate::version::Version::from_str(&start_cmd.version) {
                Ok(v) => v,
                Err(_) => {
                    errorln!("Error parsing provided version!");
                    return;
                }
            };

            if !check_notes_file(&start_cmd.notes) {
                return;
            }

            if let Some(s) = &release.state {
                warnln!("On-going release detected!");
                if s.release_version == version {
                    infoln!("Maybe you want to 'continue' instead?");
                } else {
                    infoln!(format!(
                        "Detected version {}, attempting to start {}!",
                        s.release_version, version
                    ));
                }
                return;
            }

            match crate::release::start::start(&mut release, &version, &start_cmd.notes) {
                Ok(()) => {
                    successln!(format!(
                        "Release for version {} successfully started!",
                        &version
                    ));
                }
                Err(()) => {
                    boomln!("Error starting new release!");
                }
            };
        }
        Cmds::Continue(continue_cmd) => {
            let cmd_relver = match &continue_cmd.version {
                None => None,
                Some(v) => match Version::from_str(&v) {
                    Ok(r) => Some(r),
                    Err(()) => {
                        boomln!(format!("Unable to parse provided version '{}'", v));
                        return;
                    }
                },
            };

            if let Some(v) = &release.state {
                if cmd_relver.is_some() {
                    errorln!(format!(
                        "Release state already found for version {}, but version provided.",
                        v.release_version
                    ));
                    return;
                }
            }

            let relver = match &release.state {
                None => match cmd_relver {
                    None => {
                        errorln!("Must provide a version to continue, or start a new release!");
                        return;
                    }
                    Some(v) => v,
                },
                Some(s) => s.release_version.clone(),
            };

            if !check_notes_file(&continue_cmd.notes) {
                return;
            }

            infoln!(format!("Continue a release process for version {}", relver));
            match crate::release::cont::continue_release(&mut release, &relver, &continue_cmd.notes)
            {
                Ok(()) => {
                    successln!(format!("Release {} successfully continued.", relver));
                }
                Err(err) => {
                    boomln!(format!("Error continuing release: {}", err));
                }
            };
        }
        Cmds::List => {
            boomln!("Should not have reached here!");
            return;
        }
    };
}

fn check_notes_file(notes: &PathBuf) -> bool {
    if !notes.exists() {
        errorln!(format!(
            "Release Notes file at '{}; does not exist!",
            notes.display()
        ));
        return false;
    }
    match notes.extension() {
        Some(ext) => {
            if ext.to_ascii_lowercase() != "md" {
                errorln!("Provided Release Notes file is not a Markdown file!");
                return false;
            }
        }
        None => {
            errorln!("Provided Release Notes file is not a Markdown file!");
            return false;
        }
    };
    return true;
}
