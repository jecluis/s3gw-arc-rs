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

use std::{
    collections::BTreeMap,
    fmt::{Display, Write},
};

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

pub struct StatusTable {
    pub entries: BTreeMap<u64, StatusTableEntry>,
}

pub struct StatusTableEntry {
    pub version: Version,
    pub records: Vec<String>,
}

impl Default for StatusTable {
    fn default() -> Self {
        StatusTable {
            entries: BTreeMap::new(),
        }
    }
}

impl Display for StatusTable {
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut longest_line = 0;
        for entry in self.entries.values() {
            let max_len = entry
                .records
                .iter()
                .map(|rec| {
                    let stripped = cleanup_formatted_str(&rec);
                    stripped.len() // line length
                + 15 // prefix length
                + 3 // spacing
                + 2 // buffer
                })
                .max()
                .unwrap_or(0);
            if max_len > longest_line {
                longest_line = max_len;
            }
        }
        let divider_strong = (0..longest_line).map(|_| '\u{2500}').collect::<String>();
        let divider_light = (0..longest_line).map(|_| '\u{254c}').collect::<String>();

        f.write_char('\n').unwrap();
        f.write_str(&divider_strong).unwrap();
        f.write_char('\n').unwrap();
        for entry in self.entries.values() {
            let mut output_version = true;
            for rec in &entry.records {
                let ver_str = if output_version {
                    output_version = false;
                    format!("v{}", entry.version.get_version_str())
                } else {
                    String::new()
                };

                if let Err(err) = f.write_fmt(format_args!("{:15}   {}\n", ver_str, rec)) {
                    return Err(err);
                }
            }
            f.write_str(&divider_light).unwrap();
            f.write_char('\n').unwrap();
        }

        Ok(())
    }
}

/// Clean up a formatted string, removing all terminal ANSI characters that
/// allow for the coloring and stuff.
///
fn cleanup_formatted_str(s: &String) -> String {
    let mut in_format = false;
    let mut final_str = String::new();

    for c in s.chars() {
        if c.is_control() {
            in_format = true;
            continue;
        }
        if in_format {
            if c == 'm' {
                in_format = false;
            }
            continue;
        }
        final_str.push(c);
    }

    final_str
}

impl StatusTable {
    pub fn new_entry(self: &mut Self, ver: &Version) -> &mut StatusTableEntry {
        let entry = StatusTableEntry::new(&ver);
        self.entries.insert(ver.get_version_id(), entry);
        self.entries.get_mut(&ver.get_version_id()).unwrap()
    }

    pub fn _add_record(self: &mut Self, ver: &Version, rec: &String) {
        let verid = ver.get_version_id();
        let entry = if !self.entries.contains_key(&verid) {
            self.new_entry(ver)
        } else {
            self.entries.get_mut(&verid).unwrap()
        };
        entry.add_record(&rec);
    }
}

impl StatusTableEntry {
    pub fn new(ver: &Version) -> StatusTableEntry {
        StatusTableEntry {
            version: ver.clone(),
            records: vec![],
        }
    }

    pub fn add_record(self: &mut Self, rec: &String) {
        self.records.push(rec.clone());
    }
}
