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

use crate::ws::errors::WorkspaceError;

use super::errors::WorkspaceResult;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSGitHubConfig {
    pub org: String,
    pub repo: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSGitRepoConfigValues {
    pub github: Option<WSGitHubConfig>,
    pub readonly: String,
    pub readwrite: String,
    pub tag_pattern: String,
    pub release_branch_pattern: String,
    pub final_branch_pattern: Option<String>,
    pub tag_format: String,
    pub release_branch_format: String,
    pub final_branch_format: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSGitReposConfig {
    pub s3gw: WSGitRepoConfigValues,
    pub ceph: WSGitRepoConfigValues,
    pub ui: WSGitRepoConfigValues,
    pub charts: WSGitRepoConfigValues,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSQuayRegistryConfig {
    pub s3gw: String,
    pub ui: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSUserConfig {
    pub name: String,
    pub email: String,
    pub signing_key: String,
    pub github_token: String,
}

impl Default for WSUserConfig {
    fn default() -> Self {
        WSUserConfig {
            name: String::new(),
            email: String::new(),
            signing_key: String::new(),
            github_token: String::new(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSConfig {
    pub user: WSUserConfig,
    pub git: WSGitReposConfig,
    pub registry: Option<WSQuayRegistryConfig>,
}

impl Default for WSConfig {
    fn default() -> Self {
        WSConfig {
            user: WSUserConfig::default(),
            git: WSGitReposConfig {
                s3gw: WSGitRepoConfigValues {
                    github: Some(WSGitHubConfig {
                        org: "aquarist-labs".into(),
                        repo: "s3gw".into(),
                    }),
                    readonly: String::from("https://github.com/aquarist-labs/s3gw.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw.git"),
                    tag_pattern: String::from(r"^v(\d+\.\d+\.\d+.*)$"),
                    release_branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    final_branch_pattern: None,
                    tag_format: String::from("v{{major}}.{{minor}}.{{patch}}"),
                    release_branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                    final_branch_format: None,
                },
                ceph: WSGitRepoConfigValues {
                    github: Some(WSGitHubConfig {
                        org: "aquarist-labs".into(),
                        repo: "ceph".into(),
                    }),
                    readonly: String::from("https://github.com/aquarist-labs/ceph.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/ceph.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    release_branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    final_branch_pattern: None,
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    release_branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                    final_branch_format: None,
                },
                ui: WSGitRepoConfigValues {
                    github: Some(WSGitHubConfig {
                        org: "aquarist-labs".into(),
                        repo: "s3gw-ui".into(),
                    }),
                    readonly: String::from("https://github.com/aquarist-labs/s3gw-ui.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw-ui.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    release_branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    final_branch_format: None,
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    release_branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                    final_branch_pattern: None,
                },
                charts: WSGitRepoConfigValues {
                    github: Some(WSGitHubConfig {
                        org: "aquarist-labs".into(),
                        repo: "s3gw-charts".into(),
                    }),
                    readonly: String::from("https://github.com/aquarist-labs/s3gw-charts.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw-charts.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    release_branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    final_branch_pattern: Some(String::from(r"^v(\d+\.\d+)$")),
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    release_branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                    final_branch_format: Some(String::from("v{{major}}.{{minor}}")),
                },
            },
            registry: Some(WSQuayRegistryConfig {
                s3gw: "s3gw/s3gw".into(),
                ui: "s3gw/s3gw-ui".into(),
            }),
        }
    }
}

impl WSConfig {
    /// Write current config to 'path'. The file will be created if it does not exist.
    ///
    pub fn write(self: &Self, path: &PathBuf) -> WorkspaceResult<()> {
        let f = match std::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
        {
            Ok(v) => v,
            Err(err) => {
                log::error!(
                    "Error opening file for write at {}: {}",
                    path.display(),
                    err
                );
                return Err(WorkspaceError::ConfigError);
            }
        };

        match serde_json::to_writer_pretty(f, &self) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Unable to write config to {}: {}", path.display(), err);
                return Err(WorkspaceError::ConfigError);
            }
        };
        Ok(())
    }

    /// Read config at 'path', returning a 'WSConfig' if it exists.
    ///
    pub fn read(path: &PathBuf) -> WorkspaceResult<WSConfig> {
        let f = match std::fs::File::open(path) {
            Ok(v) => v,
            Err(err) => {
                log::error!("Error opening config at {}: {}", path.display(), err);
                return Err(WorkspaceError::ConfigError);
            }
        };
        let cfg: WSConfig = match serde_json::from_reader(f) {
            Ok(v) => v,
            Err(err) => {
                log::error!("Error reading config from {}: {}", path.display(), err);
                return Err(WorkspaceError::ConfigError);
            }
        };
        Ok(cfg)
    }
}
