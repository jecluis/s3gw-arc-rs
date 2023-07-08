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

use std::collections::{HashMap, HashSet};

use tabled::settings::{Merge, Style};

use crate::ws::{version::Version, workspace::Workspace};

use super::Release;

impl Release {
    /// List releases in a given workspace 'ws'.
    ///
    pub fn list(ws: &Workspace) {
        log::info!("List releases on workspace");

        // sync workspace first
        match ws.sync() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error synchronizing workspace!");
                return;
            }
        };

        // obtain repositories
        let repos = ws.repos.as_list();
        let mut repo_names: Vec<String> = vec![];

        let mut release_per_repo: HashMap<String, HashMap<String, HashSet<String>>> =
            HashMap::new();

        let mut versions_per_release: HashMap<String, Vec<Version>> = HashMap::new();
        let mut release_versions: Vec<Version> = vec![];

        for repo in repos {
            repo_names.push(repo.name.clone());
            let versions = match repo.get_release_versions() {
                Ok(v) => v,
                Err(_) => {
                    log::error!("Error obtaining release versions for repo {}", repo.name);
                    return;
                }
            };

            release_per_repo.insert(repo.name.clone(), HashMap::new());
            let repo_hm = &mut release_per_repo.get_mut(&repo.name).unwrap();

            for release in &versions {
                if !release_versions.contains(&release.release_version) {
                    release_versions.push(release.release_version.clone());
                }
                let relver = release.release_version.get_release_version_str();
                if !versions_per_release.contains_key(&relver) {
                    versions_per_release.insert(relver.clone(), vec![]);
                }

                if !repo_hm.contains_key(&relver) {
                    repo_hm.insert(relver.clone(), HashSet::new());
                }

                let rel_hs = &mut repo_hm.get_mut(&relver).unwrap();

                let ver_vec = &mut versions_per_release.get_mut(&relver).unwrap();
                for ver in &release.versions {
                    let v = ver.clone();
                    if !ver_vec.contains(&v) {
                        ver_vec.push(v);
                    }
                    let v_str = ver.get_version_str();
                    if !rel_hs.contains(&v_str) {
                        rel_hs.insert(v_str.clone());
                    }
                }
            }
        }

        release_versions.sort_by_key(|e: &Version| e.get_version_id());

        let mut rows: Vec<(String, Vec<Vec<String>>)> = vec![];

        for relver in release_versions {
            let mut rel_rows: Vec<Vec<String>> = vec![];

            let relver_str = relver.get_release_version_str();
            let mut rel_version_entries = versions_per_release.get(&relver_str).unwrap().clone();
            rel_version_entries.sort_by_key(|e: &Version| e.get_version_id());

            for rel_version_entry_value in rel_version_entries {
                let rel_version_entry_str = rel_version_entry_value.get_version_str();

                let mut row: Vec<String> = vec![];
                for entry in &release_per_repo {
                    if let Some(repo_rel) = entry.1.get(&relver_str) {
                        if repo_rel.contains(&rel_version_entry_str) {
                            row.push(rel_version_entry_str.clone());
                        } else {
                            row.push(String::from("-"));
                        }
                    } else {
                        row.push(String::from("-"));
                    }
                }
                rel_rows.push(row);
            }
            rows.push((relver_str, rel_rows));
        }

        let mut builder = tabled::builder::Builder::default();
        let headers = std::iter::once(String::from("release")).chain(repo_names);
        builder.set_header(headers);

        for (relver_str, relver_rows) in rows {
            for row in relver_rows {
                let entry = std::iter::once(relver_str.clone()).chain(row);
                builder.push_record(entry);
            }
        }

        let mut table = builder.build();
        table.with(Merge::vertical()).with(Style::modern());
        println!("{}", table);
    }
}
