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

use crate::{errorln, infoln, version::Version};

use super::Release;

/// Synchronize existing state, including repositories and branches, for the
/// specified release. This may mean fetching release branches, checking out
/// release branches, and synchronizing submodules.
///
pub fn sync(release: &Release, relver: &Version) -> Result<(), ()> {
    infoln!("Synchronize state for release {}", relver);

    let ws = &release.ws;
    let base_ver = relver.get_base_version();

    for repo in ws.repos.as_vec() {
        log::debug!(
            "sync for release, repo '{}' base ver '{}'",
            repo.name,
            base_ver
        );

        // synchronize the repository's state with its upstream, including
        // submodules if needed.
        match repo.update(repo.update_submodules) {
            Ok(()) => {
                log::debug!("sync for release, repo '{}' sync'ed", repo.name);
            }
            Err(err) => {
                errorln!("Unable to synchronize repository '{}': {}", repo.name, err);
                return Err(());
            }
        };

        // checkout base version branch for the specified release version, for a
        // given repository.
        match repo.checkout_version_branch(&base_ver) {
            Ok(()) => {
                log::debug!(
                    "sync for release, repo '{}' checked out base ver '{}'",
                    repo.name,
                    base_ver
                );
            }
            Err(err) => {
                errorln!(
                    "Unable to checkout branch for version '{}' on repository '{}': {}",
                    base_ver,
                    repo.name,
                    err
                );
                return Err(());
            }
        };
    }
    log::debug!("finished synchronizing release '{}'", relver);

    Ok(())
}
