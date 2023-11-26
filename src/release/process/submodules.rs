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

use crate::{
    errorln,
    version::Version,
    ws::{repository::Repository, workspace::Workspace},
};

/// Represents information about a given submodule in the 's3gw' repository.
pub struct SubmoduleInfo<'a> {
    /// submodule name
    pub name: String,
    /// submodule repository
    pub repo: &'a Repository,
}

impl SubmoduleInfo<'_> {
    /// Creates information about a given submodule
    ///
    pub fn new<'a>(name: &str, repo: &'a Repository) -> SubmoduleInfo<'a> {
        SubmoduleInfo {
            name: String::from(name),
            repo: &repo,
        }
    }
}

/// Obtain all known 's3gw' repository submodules
///  note(joao): not all submodules, actually - we miss the COSI submodules, but
///  we're not handling those right now.
pub fn get_submodules<'b>(ws: &'b Workspace) -> Vec<SubmoduleInfo> {
    vec![
        SubmoduleInfo::new("ui", &ws.repos.ui),
        SubmoduleInfo::new("charts", &ws.repos.charts),
        SubmoduleInfo::new("ceph", &ws.repos.ceph),
    ]
}

/// Update a given 's3gw' repository's submodule to the specified tag version.
///
pub fn update_submodule(
    ws: &Workspace,
    info: &SubmoduleInfo,
    tagver: &Version,
) -> Result<Option<PathBuf>, ()> {
    let tagver_str = tagver.to_rc_str_fmt(&info.repo.config.tag_format);
    log::trace!("update submodule '{}' to tag '{}'", info.name, tagver_str);

    match ws
        .repos
        .s3gw
        .set_submodule_head(&info.name, &tagver_str, true)
    {
        Ok(p) => {
            match p {
                Some(_) => {
                    log::debug!("Updated submodule '{}' head to '{}'", info.name, tagver_str)
                }
                None => log::debug!("Submodule '{}' not changed", info.name),
            };
            Ok(p)
        }
        Err(err) => {
            errorln!(
                "Error updating submodule '{}' head to '{}': {}",
                info.name,
                tagver_str,
                err
            );
            Err(())
        }
    }
}

/// Update all 's3gw' repository's submodules to the specified tag version.
///
pub fn update_submodules(ws: &Workspace, tagver: &Version) -> Result<Vec<PathBuf>, ()> {
    let submodules = get_submodules(&ws);
    let mut res_vec: Vec<PathBuf> = vec![];

    for entry in &submodules {
        match update_submodule(&ws, &entry, &tagver) {
            Ok(r) => {
                if let Some(p) = r {
                    res_vec.push(p);
                }
            }
            Err(()) => {
                log::error!("Error updating submodule '{}' to '{}'", entry.name, tagver);
                return Err(());
            }
        };
    }

    Ok(res_vec)
}
