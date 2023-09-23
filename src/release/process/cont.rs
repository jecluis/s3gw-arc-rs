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

use crate::release::process::start;
use crate::release::sync;
use crate::release::Release;
use crate::successln;
use crate::{errorln, infoln, release::errors::ReleaseError, version::Version};

pub fn continue_release(
    release: &mut Release,
    version: &Version,
    notes: &PathBuf,
) -> Result<(), ReleaseError> {
    // 1. check whether release has been finished
    // 2. check whether release has been started
    // 3. sync repositories for the specified release
    // 4. start a new release candidate

    let ws = &release.ws;

    let release_versions = crate::release::common::get_release_versions(&ws, &version);
    if release_versions.contains_key(&version.get_version_id()) {
        errorln!(format!("Release version {} already exists", version));
        return Err(ReleaseError::ReleaseExistsError);
    } else if release_versions.len() == 0 {
        errorln!("Release has not been started yet.");
        return Err(ReleaseError::NotStartedError);
    }

    infoln!(format!("Continuing release {}", version));

    match sync::sync(&release, &version) {
        Ok(()) => {}
        Err(()) => {
            errorln!("Unable to sync release!");
            return Err(ReleaseError::UnknownError);
        }
    };

    match start::start_release_candidate(&ws, &version, Some(&notes)) {
        Ok(v) => {
            successln!(format!("Continued release, created {}", v));
        }
        Err(err) => {
            errorln!(format!("Error starting new release candidate: {}", err));
            return Err(err);
        }
    };

    Ok(())
}