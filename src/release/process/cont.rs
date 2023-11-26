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

use crate::boomln;
use crate::release::errors::ReleaseResult;
use crate::release::process::start;
use crate::release::sync;
use crate::release::Release;
use crate::successln;
use crate::{errorln, infoln, release::errors::ReleaseError, version::Version};

pub async fn continue_release(
    release: &mut Release,
    version: &Version,
    notes: &Option<PathBuf>,
    force: bool,
) -> ReleaseResult<()> {
    // Continuing a release requires to first synchronize the repositories, then
    // ensuring we can actually release. If so, we can start a new release
    // candidate.

    let ws = &release.ws;
    infoln!("Continuing release {}", version);

    match sync::sync(&release, &version) {
        Ok(()) => {}
        Err(()) => {
            errorln!("Unable to sync release!");
            return Err(ReleaseError::SyncError);
        }
    };

    if let Err(err) = super::validate::check_can_release(&ws, &version, force).await {
        boomln!("Can't continue releasing due to validation error: {}", err);
        return Err(err);
    }

    match start::start_release_candidate(&ws, &version, notes.as_ref()) {
        Ok(v) => {
            successln!("Continued release, created {}", v);
        }
        Err(err) => {
            errorln!("Error starting new release candidate: {}", err);
            return Err(err);
        }
    };

    Ok(())
}
