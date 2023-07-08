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

pub struct GitRefEntry {
    pub name: String,
    pub oid: String,
    pub is_tag: bool,
}

impl GitRefEntry {}

pub struct GitRefs {
    pub branches: Vec<GitRefEntry>,
    pub tags: Vec<GitRefEntry>,
}

impl GitRefs {
    pub fn from_remote(remote: &git2::Remote) -> Result<GitRefs, ()> {
        let mut branches: Vec<GitRefEntry> = vec![];
        let mut tags: Vec<GitRefEntry> = vec![];

        let head_re = regex::Regex::new(r"^refs/heads/(.*)$").unwrap();
        let tag_re = regex::Regex::new(r"^refs/tags/(.*)\^\{\}$").unwrap();

        let ls = match remote.list() {
            Ok(v) => v,
            Err(e) => {
                log::error!("Unable to list remote: {}", e);
                return Err(());
            }
        };
        for head in ls {
            let oid = head.oid();
            let name = head.name();

            if let Some(head_m) = head_re.captures(name) {
                branches.push(GitRefEntry {
                    name: String::from(&head_m[1]),
                    oid: oid.to_string(),
                    is_tag: false,
                });
            } else if let Some(tag_m) = tag_re.captures(name) {
                tags.push(GitRefEntry {
                    name: String::from(&tag_m[1]),
                    oid: oid.to_string(),
                    is_tag: true,
                });
            }
        }

        Ok(GitRefs { branches, tags })
    }
}
