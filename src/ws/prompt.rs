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

use inquire::{required, Confirm, Text};

use crate::ws::errors::WorkspaceError;

use super::{
    config::{
        WSConfig, WSGitHubConfig, WSGitRepoConfigValues, WSGitReposConfig, WSQuayRegistryConfig,
        WSUserConfig,
    },
    errors::WorkspaceResult,
};

/// Prompt for a specific custom git repository. This is a helper function.
///
fn prompt_custom_git_repo_value(
    name: &str,
    default: &WSGitRepoConfigValues,
) -> WorkspaceResult<Option<WSGitRepoConfigValues>> {
    match Confirm::new(&format!("Set custom URIs for {}?", name))
        .with_default(true)
        .prompt()
    {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    let ro = match Text::new(&format!("{} read-only URI:", name))
        .with_default(&default.readonly)
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    let rw = match Text::new(&format!("{} read-write URI:", name))
        .with_default(&default.readwrite)
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    Ok(Some(WSGitRepoConfigValues {
        github: None,
        readonly: ro,
        readwrite: rw,
        tag_pattern: default.tag_pattern.clone(),
        release_branch_pattern: default.release_branch_pattern.clone(),
        final_branch_pattern: default.final_branch_pattern.clone(),
        tag_format: default.tag_format.clone(),
        release_branch_format: default.release_branch_format.clone(),
        final_branch_format: default.final_branch_format.clone(),
    }))
}

/// Prompt for a custom github repository belonging to a specific organization.
/// This is a helper function.
///
fn prompt_custom_github_repo_value(
    name: &str,
    org: &String,
    default_name: &str,
    default: &WSGitRepoConfigValues,
) -> WorkspaceResult<WSGitRepoConfigValues> {
    let repo = match Text::new(&format!("{:7} at {} /", name, org))
        .with_default(&default_name)
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    let gitless_repo = match repo.find(".git") {
        None => repo.clone(),
        Some(v) => repo[..v].into(), // grab slice, drop repo's '.git' suffix
    };

    Ok(WSGitRepoConfigValues {
        github: Some(WSGitHubConfig {
            org: org.clone(),
            repo: gitless_repo.clone(),
        }),
        readonly: format!("https://github.com/{}/{}", org, repo),
        readwrite: format!("git@github.com:{}/{}", org, repo),
        tag_pattern: default.tag_pattern.clone(),
        release_branch_pattern: default.release_branch_pattern.clone(),
        final_branch_pattern: default.final_branch_pattern.clone(),
        tag_format: default.tag_format.clone(),
        release_branch_format: default.release_branch_format.clone(),
        final_branch_format: default.final_branch_format.clone(),
    })
}

/// Prompt for custom git repositories for the various tracked repositories.
///
fn prompt_custom_git_repos(default: &WSGitReposConfig) -> WorkspaceResult<WSGitReposConfig> {
    let mut cfg = default.clone();

    if match Confirm::new("From GitHub?").with_default(true).prompt() {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    } {
        let org = match Text::new("Organization:")
            .with_default("aquarist-labs")
            .prompt()
        {
            Ok(v) => v,
            Err(err) => {
                return Err(match err {
                    inquire::InquireError::OperationCanceled
                    | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                    _ => WorkspaceError::UnknownPromptError,
                });
            }
        };

        let repo_vec = vec![
            ("s3gw", "s3gw.git", &default.s3gw, &mut cfg.s3gw),
            ("s3gw-ui", "s3gw-ui.git", &default.ui, &mut cfg.ui),
            (
                "charts",
                "s3gw-charts.git",
                &default.charts,
                &mut cfg.charts,
            ),
            ("ceph", "ceph.git", &default.ceph, &mut cfg.ceph),
        ];

        for entry in repo_vec {
            match prompt_custom_github_repo_value(entry.0, &org, entry.1, &entry.2) {
                Ok(v) => {
                    let tgt = entry.3;
                    *tgt = v;
                }
                Err(err) => return Err(err),
            };
        }

        log::trace!("{}", serde_json::to_string_pretty(&cfg).unwrap());
        return Ok(cfg);
    }

    let repo_vec = vec![
        ("s3gw", &default.s3gw, &mut cfg.s3gw),
        ("s3gw-ui", &default.ui, &mut cfg.ui),
        ("charts", &default.charts, &mut cfg.charts),
        ("ceph", &default.ceph, &mut cfg.ceph),
    ];

    for entry in repo_vec {
        match prompt_custom_git_repo_value(entry.0, entry.1) {
            Ok(None) => {}
            Ok(Some(v)) => {
                let tgt = entry.2;
                *tgt = v;
            }
            Err(err) => return Err(err),
        };
    }

    Ok(cfg)
}

/// Prompt for quay registries for deliverable artifacts.
///
fn prompt_registries(default: &WSQuayRegistryConfig) -> WorkspaceResult<WSQuayRegistryConfig> {
    let s3gw = match prompt_single_registry_repo(&"s3gw".into(), &default.s3gw) {
        Ok(v) => v,
        Err(err) => return Err(err),
    };
    let ui = match prompt_single_registry_repo(&"s3gw-ui".into(), &default.ui) {
        Ok(v) => v,
        Err(err) => return Err(err),
    };

    Ok(WSQuayRegistryConfig { s3gw, ui })
}

/// Prompt for a single repository's location (i.e., namespace/repository).
///
fn prompt_single_registry_repo(name: &String, default_repo: &String) -> WorkspaceResult<String> {
    let repo = match Text::new(&format!("{:7} at quay.io/", name))
        .with_default(&default_repo)
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    Ok(repo)
}

/// Prompt for user-related informations, such as the user's name, email, etc.
///
fn prompt_user() -> WorkspaceResult<WSUserConfig> {
    let name = match Text::new("User Name:").with_validator(required!()).prompt() {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };
    let email = match Text::new("User email:")
        .with_validator(|v: &str| {
            let re = regex::Regex::new(r"^[\w_\-.]+@[\w\-_.]+$").unwrap();
            if re.is_match(&v) {
                return Ok(inquire::validator::Validation::Valid);
            }
            Ok(inquire::validator::Validation::Invalid(
                "must be an email address".into(),
            ))
        })
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };
    let signing_key = match Text::new("Signing key:")
        .with_validator(required!())
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };
    let ghtoken = match Text::new("GitHub token:")
        .with_validator(|v: &str| {
            let re = regex::Regex::new(r"^ghp_\w+$").unwrap();
            if re.is_match(v) {
                return Ok(inquire::validator::Validation::Valid);
            }
            Ok(inquire::validator::Validation::Invalid(
                "wrong token format".into(),
            ))
        })
        .prompt()
    {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationCanceled
                | inquire::InquireError::OperationInterrupted => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownPromptError,
            });
        }
    };

    Ok(WSUserConfig {
        name,
        email,
        signing_key,
        github_token: ghtoken,
    })
}

/// Prompt the user for values required to initiate a new workspace.
///
pub fn init_prompt(default_config: &WSConfig) -> WorkspaceResult<WSConfig> {
    let mut cfg = default_config.clone();

    match prompt_user() {
        Ok(v) => cfg.user = v,
        Err(err) => return Err(err),
    };

    match Confirm::new("Do you want to setup custom git repositories?")
        .with_default(true)
        .prompt()
    {
        Ok(true) => {
            match prompt_custom_git_repos(&default_config.git) {
                Ok(v) => cfg.git = v,
                Err(err) => return Err(err),
            };
        }
        Ok(false) => {}
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationInterrupted
                | inquire::InquireError::OperationCanceled => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownError,
            });
        }
    };

    log::trace!(
        "final cfg = {}",
        serde_json::to_string_pretty(&cfg.git).unwrap()
    );

    match Confirm::new("Use Quay as the registry?")
        .with_default(true)
        .prompt()
    {
        Ok(false) => {
            cfg.registry = None;
        }
        Ok(true) => {
            let default_registry = default_config.registry.as_ref().unwrap();
            cfg.registry = match prompt_registries(&default_registry) {
                Ok(v) => Some(v),
                Err(err) => return Err(err),
            }
        }
        Err(err) => {
            return Err(match err {
                inquire::InquireError::OperationInterrupted
                | inquire::InquireError::OperationCanceled => WorkspaceError::UserAborted,
                _ => WorkspaceError::UnknownError,
            });
        }
    };

    Ok(cfg)
}
