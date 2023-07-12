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

use inquire::{Confirm, Text};

use crate::{
    release::{ReleaseError, ReleaseState},
    ws::{version::Version, workspace::Workspace},
};

use super::Release;

impl Release {
    /// Initiate a release in the given workspace.
    ///
    pub fn init(ws: &Workspace, version_str: &Option<String>) -> Result<Release, ReleaseError> {
        let mut release = match Release::open(&ws) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Error opening release, unable to init!");
                return Err(ReleaseError::InitError);
            }
        };

        if let Some(state) = release.state {
            log::error!("Workspace already has a release initiated!");
            println!(
                "Workspace already initiated for release {}.",
                state.release_version
            );
            return Err(ReleaseError::AlreadyInit);
        }

        let version: Version = if let Some(v) = version_str {
            match Version::from_str(&v) {
                Ok(r) => r,
                Err(_) => {
                    log::error!("Unable to parse provided version string!");
                    return Err(ReleaseError::InitError);
                }
            }
        } else {
            match init_prompt() {
                Ok(v) => v,
                Err(_) => {
                    log::error!("Unable to init release!");
                    return Err(ReleaseError::InitError);
                }
            }
        };

        log::debug!("init version {}", version);

        match ws.sync() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error synchronizing workspace!");
                return Err(ReleaseError::InitError);
            }
        };

        let release_versions = match ws.repos.s3gw.get_release_versions() {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to obtain release versions for s3gw repo");
                return Err(ReleaseError::InitError);
            }
        };

        for ver in release_versions {
            if ver.release_version == version {
                log::debug!("Release already exists!");
                match prompt_release_exists() {
                    Ok(true) => {
                        break;
                    }
                    Ok(false) => {
                        log::debug!("abort release init!");
                        return Err(ReleaseError::UserAborted);
                    }
                    Err(_) => {
                        return Err(ReleaseError::InitError);
                    }
                };
            }
        }

        release.state = Some(ReleaseState {
            release_version: version,
        });
        match release.write() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Unable to write release state!");
                return Err(ReleaseError::InitError);
            }
        };
        Ok(release)
    }
}

fn init_prompt() -> Result<Version, ReleaseError> {
    let version_str = match Text::new("release version:")
        .with_help_message("MAJOR.minor; e.g., 0.17")
        .with_validator(|v: &str| match Version::from_str(&String::from(v)) {
            Ok(r) => {
                if r.patch.is_some() || r.rc.is_some() {
                    Ok(inquire::validator::Validation::Invalid(
                        "must be in MAJOR.minor format".into(),
                    ))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            }
            Err(_) => Ok(inquire::validator::Validation::Invalid(
                "Unable to parse version".into(),
            )),
        })
        .prompt()
    {
        Ok(v) => v,
        Err(_) => {
            log::error!("Unable to obtain version string from user");
            return Err(ReleaseError::InitError);
        }
    };

    match Version::from_str(&version_str) {
        Ok(v) => Ok(v),
        Err(_) => {
            log::error!("Unable to obtain version from '{}'", version_str);
            return Err(ReleaseError::InitError);
        }
    }
}

fn prompt_release_exists() -> Result<bool, ()> {
    match Confirm::new("Release already exists. Continue?")
        .with_default(false)
        .prompt()
    {
        Ok(v) => Ok(v),
        Err(e) => {
            log::error!("Error prompting user: {}", e);
            return Err(());
        }
    }
}
