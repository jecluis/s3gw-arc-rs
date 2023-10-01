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

use std::collections::BTreeMap;

use crate::{
    version::Version,
    ws::{repository::Repository, workspace::Workspace},
};

/// Obtains versions corresponding to release 'relver' from the 's3gw' repository.
///
pub fn get_release_versions(ws: &Workspace, relver: &Version) -> BTreeMap<u64, Version> {
    get_release_versions_from_repo(&ws.repos.s3gw, &relver)
}

/// Obtain versions corresponding to release 'relver' from the provided repository.
///
pub fn get_release_versions_from_repo(
    repo: &Repository,
    relver: &Version,
) -> BTreeMap<u64, Version> {
    let min_id = relver.min().get_version_id();
    let max_id = relver.max().get_version_id();

    let version_tree = &repo.get_versions().unwrap();
    let avail = version_tree.range((
        std::ops::Bound::Included(min_id),
        std::ops::Bound::Included(max_id),
    ));

    let mut versions = BTreeMap::<u64, Version>::new();
    for (vid, v) in avail {
        versions.insert(vid.clone(), v.clone());
    }

    versions
}
