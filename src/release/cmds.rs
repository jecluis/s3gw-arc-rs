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

use super::ReleaseState;

#[derive(Clone, Copy, Debug)]
pub enum CmdVersionError {
    UnableToParseError,
    StateFoundError,
    VersionNotProvidedError,
}

#[derive(clap::Subcommand)]
pub enum Cmds {
    /// List releases.
    List,
    /// Release status.
    Status(StatusCommand),
    /// Sync release state.
    Sync(SyncCommand),
    /// Start a new release process.
    Start(StartCommand),
    /// Continue the release process.
    Continue(ContinueCommand),
    /// Finish the release process.
    Finish(FinishCommand),

    /// Generate release announcement.
    Announce(AnnounceCommand),
}

#[derive(clap::Args)]
pub struct StatusCommand {
    /// Version for which to obtain status
    #[arg(value_name = "VERSION", short, long)]
    version: Option<String>,
}

#[derive(clap::Args)]
pub struct SyncCommand {
    /// Version for which to sync the release
    #[arg(value_name = "VERSION", short, long)]
    version: String,
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
    notes: Option<PathBuf>,

    /// Release version to continue (e.g., v0.17.1)
    #[arg(value_name = "VERSION", short, long)]
    version: Option<String>,

    /// Force continuing a release regardless of previous candidate state
    #[arg(short, long)]
    force: bool,
}

#[derive(clap::Args)]
pub struct FinishCommand {
    /// Release version to finish (e.g., v0.17.1)
    #[arg(value_name = "VERSION", short, long)]
    version: Option<String>,

    /// Force finishing a release regardless of previous candidae state
    #[arg(short, long)]
    force: bool,
}

#[derive(clap::Args)]
pub struct AnnounceCommand {
    /// Release version to announce (e.g., v0.17.1)
    #[arg(value_name = "VERSION", short, long)]
    version: String,

    #[arg(value_name = "FILE", short, long)]
    outfile: Option<PathBuf>,
}

pub async fn handle_cmds(cmd: &Cmds) {
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
        Cmds::Status(status_cmd) => {
            log::debug!("Obtain release status");
            let version = match check_version_against_state(&release.state, &status_cmd.version) {
                Ok(v) => v,
                Err(CmdVersionError::VersionNotProvidedError) => {
                    errorln!("Must provide a version, or have a release state initiated!");
                    return;
                }
                Err(_) => {
                    // all other errors are output by the check functions.
                    return;
                }
            };
            release.status(&version).await;
        }
        Cmds::Sync(sync_cmd) => {
            log::debug!("Synchronize release state");
            let version = match Version::from_str(&sync_cmd.version) {
                Ok(v) => v,
                Err(()) => {
                    errorln!("Error parsing provided version!");
                    return;
                }
            };
            match crate::release::sync::sync(&release, &version) {
                Ok(()) => {
                    successln!(
                        "Successfully synchronized release state for version '{}'",
                        version
                    );
                }
                Err(()) => {
                    errorln!(
                        "Error synchronizing release state for version '{}'",
                        version
                    );
                }
            }
        }
        Cmds::Start(start_cmd) => {
            infoln!(
                "Start a new release process for version {}",
                start_cmd.version
            );
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
                    infoln!(
                        "Detected version {}, attempting to start {}!",
                        s.release_version,
                        version
                    );
                }
                return;
            }

            match crate::release::process::start::start(&mut release, &version, &start_cmd.notes) {
                Ok(()) => {
                    successln!("Release for version {} successfully started!", &version);
                }
                Err(err) => {
                    boomln!("Error starting new release: {}", err);
                }
            };
        }
        Cmds::Continue(continue_cmd) => {
            let relver = match check_version_against_state(&release.state, &continue_cmd.version) {
                Ok(v) => v,
                Err(CmdVersionError::VersionNotProvidedError) => {
                    errorln!("Must provide a version to continue, or have a started release!");
                    return;
                }
                Err(_) => {
                    // all other errors are output by the check function.
                    return;
                }
            };

            if let Some(n) = &continue_cmd.notes {
                if !check_notes_file(&n) {
                    return;
                }
            }

            infoln!("Continue a release process for version {}", relver);
            match crate::release::process::cont::continue_release(
                &mut release,
                &relver,
                &continue_cmd.notes,
                continue_cmd.force,
            )
            .await
            {
                Ok(()) => {
                    successln!("Release {} successfully continued.", relver);
                }
                Err(err) => {
                    boomln!("Error continuing release: {}", err);
                }
            };
        }
        Cmds::Finish(finish_cmd) => {
            let relver = match check_version_against_state(&release.state, &finish_cmd.version) {
                Ok(v) => v,
                Err(CmdVersionError::VersionNotProvidedError) => {
                    errorln!("Must provide a version to finish, or have a started release!");
                    return;
                }
                Err(_) => {
                    // all other errors are output by the check function.
                    return;
                }
            };

            infoln!("Finish release process for version {}", relver);
            match crate::release::process::finish::finish(&mut release, &relver, finish_cmd.force)
                .await
            {
                Ok(()) => {
                    successln!("Finished release {}!", relver);
                }
                Err(err) => {
                    boomln!("Error finishing release: {}", err);
                }
            };
        }
        Cmds::Announce(announce_cmd) => {
            let relver = match Version::from_str(&announce_cmd.version) {
                Err(()) => {
                    boomln!(
                        "Unable to parse provided version '{}'",
                        announce_cmd.version
                    );
                    return;
                }
                Ok(v) => v,
            };

            match crate::release::process::announce::announce(
                &mut release,
                &relver,
                &announce_cmd.outfile,
            ) {
                Ok(()) => {}
                Err(err) => {
                    boomln!("Error announcing release '{}': {}", relver, err);
                    return;
                }
            }
        }
        Cmds::List => {
            boomln!("Should not have reached here!");
            return;
        }
    };
}

fn check_notes_file(notes: &PathBuf) -> bool {
    if !notes.exists() {
        errorln!(
            "Release Notes file at '{}; does not exist!",
            notes.display()
        );
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

fn check_version_against_state(
    state: &Option<ReleaseState>,
    version: &Option<String>,
) -> Result<Version, CmdVersionError> {
    let cmd_relver = match &version {
        None => None,
        Some(v) => match Version::from_str(v) {
            Ok(r) => Some(r),
            Err(()) => {
                boomln!("Unable to parse provided version '{}'", v);
                return Err(CmdVersionError::UnableToParseError);
            }
        },
    };

    if let Some(v) = &state {
        if cmd_relver.is_some() {
            errorln!(
                "Release state already found for version {}, but version provided.",
                v.release_version
            );
            return Err(CmdVersionError::StateFoundError);
        }
    }

    let relver = match &state {
        None => match cmd_relver {
            None => {
                return Err(CmdVersionError::VersionNotProvidedError);
            }
            Some(v) => v,
        },
        Some(s) => s.release_version.clone(),
    };

    Ok(relver)
}
