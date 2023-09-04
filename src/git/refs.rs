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

use std::fmt::Display;

pub enum GitRefType {
    BRANCH,
    TAG,
}

impl Display for GitRefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match &self {
            GitRefType::BRANCH => "branch",
            GitRefType::TAG => "tag",
        })
    }
}

pub struct GitRefEntry {
    pub name: String,
    pub oid: String,
    pub reftype: GitRefType,
    pub is_remote: bool,
}

impl GitRefEntry {
    pub fn is_branch(self: &Self) -> bool {
        match self.reftype {
            GitRefType::BRANCH => true,
            _ => false,
        }
    }

    pub fn is_tag(self: &Self) -> bool {
        match self.reftype {
            GitRefType::TAG => true,
            _ => false,
        }
    }
}

pub struct GitRefs {
    pub branches: Vec<GitRefEntry>,
    pub tags: Vec<GitRefEntry>,
}

impl GitRefs {}

/// Obtain head references from a remote.
///
pub fn get_refs_from_remote(remote: &git2::Remote) -> Result<Vec<GitRefEntry>, ()> {
    let mut ref_vec: Vec<GitRefEntry> = vec![];
    let head_re = regex::Regex::new(r"^refs/heads/(.*)$").unwrap();

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
            ref_vec.push(GitRefEntry {
                name: String::from(&head_m[1]),
                oid: oid.to_string(),
                reftype: GitRefType::BRANCH,
                is_remote: true,
            });
        }
    }

    Ok(ref_vec)
}

/// Obtain tag and head references from a local repository.
///
pub fn get_refs_from_local(repository: &git2::Repository) -> Result<Vec<GitRefEntry>, ()> {
    let mut ref_vec: Vec<GitRefEntry> = vec![];
    let tag_re = regex::Regex::new(r"^refs/tags/(.*)$").unwrap();
    let heads_re = regex::Regex::new(r"^refs/heads/(.*)$").unwrap();

    let ref_it = match repository.references() {
        Ok(r) => r,
        Err(err) => {
            log::error!("Unable to obtain local repository references: {}", err);
            return Err(());
        }
    };

    fn get_name(n: &str, re: &regex::Regex) -> String {
        if let Some(m) = re.captures(n) {
            String::from(&m[1])
        } else {
            panic!("Unexpected error parsing name: {}", n);
        }
    }

    for entry in ref_it {
        if let Ok(r) = entry {
            let oid = match r.peel_to_commit() {
                Err(err) => {
                    log::error!(
                        "Unable to obtain commit for {}: {}",
                        r.name().unwrap_or("N/A"),
                        err
                    );
                    continue;
                }
                Ok(c) => c.id().to_string(),
            };

            log::trace!(
                "=> ref: {}, is_branch: {}, is_remote: {}",
                match r.name() {
                    None => "N/A",
                    Some(n) => n,
                },
                r.is_branch(),
                r.is_remote()
            );

            let reftype = if r.is_tag() {
                GitRefType::TAG
            } else if r.is_branch() {
                GitRefType::BRANCH
            } else {
                continue;
            };

            let name = match r.name() {
                None => {
                    continue;
                }
                Some(n) => {
                    if r.is_remote() {
                        continue;
                    } else if r.is_tag() {
                        get_name(n, &tag_re)
                    } else if r.is_branch() {
                        get_name(n, &heads_re)
                    } else {
                        continue;
                    }
                }
            };

            ref_vec.push(GitRefEntry {
                name: name.clone(),
                oid: oid.clone(),
                reftype,
                is_remote: false,
            });
        }
    }

    Ok(ref_vec)
}

/// Obtain references from a repository, both local and its remote.
///
pub fn get_refs_from(
    remote: &git2::Remote,
    repo: &git2::Repository,
) -> Result<Vec<GitRefEntry>, ()> {
    let mut ref_vec: Vec<GitRefEntry> = vec![];

    match &mut get_refs_from_remote(&remote) {
        Err(()) => {
            log::error!("Error obtaining references from remote repository!");
            return Err(());
        }
        Ok(v) => ref_vec.append(v),
    };

    match &mut get_refs_from_local(&repo) {
        Err(()) => {
            log::error!("Error obtaining references from local repository!");
            return Err(());
        }
        Ok(v) => ref_vec.append(v),
    };

    Ok(ref_vec)
}
