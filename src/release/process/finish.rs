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

use crate::{
    boomln, errorln, infoln,
    release::sync,
    release::{
        errors::ReleaseResult,
        process::{charts, start},
    },
    successln,
    version::Version,
};

use crate::release::{errors::ReleaseError, Release};

pub fn finish(release: &mut Release, version: &Version) -> ReleaseResult<()> {
    // 1. check whether release has been finished
    // 2. check whether release has been started
    // 3. sync repositories for the specified release
    // 4. find the highest release candidate
    // 5. adjust charts version
    // 6. perform the release, via start::perform_release()
    // 7. push out final release.

    let ws = &release.ws;

    let release_versions = crate::release::common::get_release_versions(&ws, &version);
    if release_versions.contains_key(&version.get_version_id()) {
        errorln!("Release version {} already exists", version);
        return Err(ReleaseError::ReleaseExistsError);
    } else if release_versions.len() == 0 {
        errorln!("Release has not been started yet.");
        return Err(ReleaseError::NotStartedError);
    }

    infoln!("Continuing release {}", version);

    match sync::sync(&release, &version) {
        Ok(()) => {}
        Err(()) => {
            errorln!("Unable to sync release!");
            return Err(ReleaseError::SyncError);
        }
    };

    let max = match release_versions.last_key_value() {
        None => {
            errorln!("Could not find the highest release candidate!");
            return Err(ReleaseError::CorruptedError);
        }
        Some((_, v)) => v,
    };
    infoln!("Basing release on highest candidate: {}", max);

    // adjust charts version

    infoln!("Update chart to version {}", version);
    if let Err(err) = charts::update_charts(&ws.repos.charts, &version) {
        boomln!("Error updating chart: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    match start::perform_release(&ws, &version, &version, &None) {
        Ok(()) => {}
        Err(err) => {
            errorln!("Unable to finish release for {}: {}", version, err);
            return Err(err);
        }
    };

    // push final chart branch
    //  This is a workaround that avoids releasing the chart until we
    //  effectively are ready to finish the release. So far we have been pushing
    //  to a "temporary" branch for the current release, but now we need to have
    //  a specific branch name in the charts repository so the release workflow
    //  can be triggered.

    infoln!("Finalizing Helm Chart release");
    if let Err(err) = charts::finalize_charts_release(&ws.repos.charts, &version) {
        errorln!("Unable to finalize chart for publishing: {}", err);
        return Err(ReleaseError::UnknownError);
    }

    successln!("Version {} released!", version);

    Ok(())
}
