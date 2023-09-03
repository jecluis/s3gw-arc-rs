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

    pub fn from_local(repo: &git2::Repository) -> Result<GitRefs, ()> {
        let mut branches: Vec<GitRefEntry> = vec![];
        let mut tags: Vec<GitRefEntry> = vec![];

        let ref_it = match repo.references() {
            Ok(r) => r,
            Err(err) => {
                log::error!("Unable to obtain local repository references: {}", err);
                return Err(());
            }
        };

        for entry in ref_it {
            if let Ok(r) = entry {
                let oid = match r.peel_to_commit() {
                    Err(err) => {
                        log::error!(
                            "Unable to obtain commit for {}: {}",
                            r.name().unwrap_or("N/A"),
                            err
                        );
                        String::from("N/A")
                    }
                    Ok(c) => c.id().to_string(),
                };
                println!(
                    "ref name: {}, is_branch: {}, is_tag: {}, is_remote: {}, oid: {}",
                    match r.name() {
                        None => "N/A",
                        Some(v) => v,
                    },
                    r.is_branch(),
                    r.is_tag(),
                    r.is_remote(),
                    oid,
                );
            }
        }

        Err(())
    }
}
