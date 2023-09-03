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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSGitRepoConfigValues {
    pub readonly: String,
    pub readwrite: String,
    pub tag_pattern: String,
    pub branch_pattern: String,
    pub tag_format: String,
    pub branch_format: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSGitReposConfig {
    pub s3gw: WSGitRepoConfigValues,
    pub ceph: WSGitRepoConfigValues,
    pub ui: WSGitRepoConfigValues,
    pub charts: WSGitRepoConfigValues,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSRegistryConfig {
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
    pub registry: WSRegistryConfig,
}

impl Default for WSConfig {
    fn default() -> Self {
        WSConfig {
            user: WSUserConfig::default(),
            git: WSGitReposConfig {
                s3gw: WSGitRepoConfigValues {
                    readonly: String::from("https://github.com/aquarist-labs/s3gw.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw.git"),
                    tag_pattern: String::from(r"^v(\d+\.\d+\.\d+.*)$"),
                    branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    tag_format: String::from("v{{major}}.{{minor}}.{{patch}}"),
                    branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                },
                ceph: WSGitRepoConfigValues {
                    readonly: String::from("https://github.com/aquarist-labs/ceph.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/ceph.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                },
                ui: WSGitRepoConfigValues {
                    readonly: String::from("https://github.com/aquarist-labs/s3gw-ui.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw-ui.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    branch_pattern: String::from(r"^s3gw-v(\d+\.\d+)$"),
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    branch_format: String::from("s3gw-v{{major}}.{{minor}}"),
                },
                charts: WSGitRepoConfigValues {
                    readonly: String::from("https://github.com/aquarist-labs/s3gw-charts.git"),
                    readwrite: String::from("git@github.com:aquarist-labs/s3gw-charts.git"),
                    tag_pattern: String::from(r"^s3gw-v(\d+\.\d+\.\d+.*)$"),
                    branch_pattern: String::from(r"^v(\d+\.\d+)$"),
                    tag_format: String::from("s3gw-v{{major}}.{{minor}}.{{patch}}"),
                    branch_format: String::from("v{{major}}.{{minor}}"),
                },
            },
            registry: WSRegistryConfig {
                s3gw: String::from("quay.io/s3gw/s3gw"),
                ui: String::from("quay.io/s3gw/s3gw-ui"),
            },
        }
    }
}

impl WSConfig {
    /// Write current config to 'path'. The file will be created if it does not exist.
    ///
    pub fn write(self: &Self, path: &PathBuf) -> Result<(), ()> {
        let f = match std::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
        {
            Ok(v) => v,
            Err(_) => {
                log::error!("Error opening file for write at {}", path.display());
                return Err(());
            }
        };

        match serde_json::to_writer_pretty(f, &self) {
            Ok(_) => {}
            Err(_) => {
                log::error!("Unable to write config to {}", path.display());
                return Err(());
            }
        };
        Ok(())
    }

    /// Read config at 'path', returning a 'WSConfig' if it exists.
    ///
    pub fn read(path: &PathBuf) -> Result<WSConfig, ()> {
        let f = match std::fs::File::open(path) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Error opening config at {}", path.display());
                return Err(());
            }
        };
        let cfg: WSConfig = match serde_json::from_reader(f) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Error reading config from {}", path.display());
                return Err(());
            }
        };
        Ok(cfg)
    }
}
