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

use tabled::settings::{Merge, Style};

use crate::version::Version;
use crate::ws::workspace::Workspace;
use crate::{boomln, errorln, infoln};

struct ReleaseVersionTreeEntry {
    pub release: Version,
    pub by_tag: BTreeMap<u64, ReleaseTagEntry>,
}

struct ReleaseTagEntry {
    pub version: Version,
    pub repos: Vec<String>,
}

/// List releases in a given workspace 'ws'.
pub fn list(ws: &Workspace) -> Result<(), ()> {
    infoln!("List releases on workspace");

    // sync workspace first
    match ws.sync() {
        Ok(()) => {}
        Err(()) => {
            boomln!("Error synchronizing workspace!");
            return Err(());
        }
    };

    let repos = ws.repos.as_vec();

    let mut version_tree = BTreeMap::<u64, ReleaseVersionTreeEntry>::new();

    for repo in &repos {
        let releases = match repo.get_releases() {
            Ok(v) => v,
            Err(err) => {
                errorln!(
                    "Unable to obtain releases for repository '{}': {}",
                    repo.name,
                    err
                );
                return Err(());
            }
        };

        for (_, base_ver) in &releases {
            let base_ver_id = base_ver.version.get_version_id();
            if !version_tree.contains_key(&base_ver_id) {
                version_tree.insert(
                    base_ver_id.clone(),
                    ReleaseVersionTreeEntry {
                        release: base_ver.version.clone(),
                        by_tag: BTreeMap::<u64, ReleaseTagEntry>::new(),
                    },
                );
            }
            let tag_tree = &mut version_tree.get_mut(&base_ver_id).unwrap().by_tag;
            for (_, release) in &base_ver.releases {
                for (_, tagver) in &release.versions {
                    let tagver_id = tagver.get_version_id();

                    if !tag_tree.contains_key(&tagver_id) {
                        tag_tree.insert(
                            tagver_id.clone(),
                            ReleaseTagEntry {
                                version: tagver.clone(),
                                repos: vec![],
                            },
                        );
                    }
                    tag_tree
                        .get_mut(&tagver_id)
                        .unwrap()
                        .repos
                        .push(repo.name.clone());
                }
            }
        }
    }

    let repo_names = repos.iter().map(|e| e.name.clone()).collect();
    print_version_table(&repo_names, &version_tree);
    Ok(())
}

fn print_version_table(
    repo_names: &Vec<String>,
    releases: &BTreeMap<u64, ReleaseVersionTreeEntry>,
) {
    let mut builder = tabled::builder::Builder::default();
    let headers = std::iter::once(String::from("release")).chain(repo_names.clone());
    builder.set_header(headers);

    for (_, relver) in releases {
        let base_ver_str = relver.release.get_base_version_str();

        for (_, tag) in &relver.by_tag {
            let mut row: Vec<String> = vec![base_ver_str.clone()];
            for repo in repo_names {
                let tagver_str = tag.version.get_version_str();
                row.push(if tag.repos.contains(repo) {
                    tagver_str.clone()
                } else {
                    "-".into()
                });
            }
            builder.push_record(row);
        }
    }

    let mut table = builder.build();
    table.with(Merge::vertical()).with(Style::modern());
    println!("{}", table);
}
