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

use super::config::{
    WSConfig, WSGitRepoConfigValues, WSGitReposConfig, WSRegistryConfig, WSUserConfig,
};

/// Prompt for a specific custom git repository. This is a helper function.
///
fn prompt_custom_git_repo_value(
    name: &str,
    default: &WSGitRepoConfigValues,
) -> Result<Option<WSGitRepoConfigValues>, ()> {
    match Confirm::new(&format!("Set custom URIs for {}?", name))
        .with_default(true)
        .prompt()
    {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(_) => return Err(()),
    };

    let ro = match Text::new(&format!("{} read-only URI:", name))
        .with_default(&default.readonly)
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };

    let rw = match Text::new(&format!("{} read-write URI:", name))
        .with_default(&default.readwrite)
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };

    Ok(Some(WSGitRepoConfigValues {
        readonly: ro,
        readwrite: rw,
    }))
}

/// Prompt for custom git repositories for the various tracked repositories.
///
fn prompt_custom_git_repos(default: &WSGitReposConfig) -> Result<WSGitReposConfig, ()> {
    let mut cfg = default.clone();

    match prompt_custom_git_repo_value("s3gw", &default.s3gw) {
        Ok(None) => {}
        Ok(Some(v)) => {
            cfg.s3gw = v;
        }
        Err(_) => return Err(()),
    };

    match prompt_custom_git_repo_value("s3gw-ui", &default.ui) {
        Ok(None) => {}
        Ok(Some(v)) => {
            cfg.ui = v;
        }
        Err(_) => return Err(()),
    };

    match prompt_custom_git_repo_value("charts", &default.charts) {
        Ok(None) => {}
        Ok(Some(v)) => {
            cfg.charts = v;
        }
        Err(_) => return Err(()),
    };

    match prompt_custom_git_repo_value("ceph", &default.ceph) {
        Ok(None) => {}
        Ok(Some(v)) => {
            cfg.ceph = v;
        }
        Err(_) => return Err(()),
    };

    Ok(cfg)
}

/// Prompt for a specific registry. This is a helper function.
///
fn prompt_custom_registry_value(name: &str, default: &String) -> Result<Option<String>, ()> {
    match Confirm::new(&format!("Set custom registry URI for {}?", name))
        .with_default(true)
        .prompt()
    {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(_) => return Err(()),
    };

    let uri = match Text::new(&format!("{} registry URI:", name))
        .with_default(&default)
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
    };
    Ok(Some(uri))
}

/// Prompt for custom registries for deliverable artifacts.
///
fn prompt_custom_registries(default: &WSRegistryConfig) -> Result<WSRegistryConfig, ()> {
    let mut cfg = default.clone();

    match prompt_custom_registry_value("s3gw", &cfg.s3gw) {
        Ok(None) => {}
        Ok(Some(v)) => cfg.s3gw = v,
        Err(_) => return Err(()),
    };

    match prompt_custom_registry_value("s3gw-ui", &cfg.ui) {
        Ok(None) => {}
        Ok(Some(v)) => cfg.ui = v,
        Err(_) => return Err(()),
    };

    Ok(cfg)
}

/// Prompt for user-related informations, such as the user's name, email, etc.
///
fn prompt_user() -> Result<WSUserConfig, ()> {
    let name = match Text::new("User Name:").with_validator(required!()).prompt() {
        Ok(v) => v,
        Err(_) => return Err(()),
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
        Err(_) => return Err(()),
    };
    let signing_key = match Text::new("Signing key:")
        .with_validator(required!())
        .prompt()
    {
        Ok(v) => v,
        Err(_) => return Err(()),
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
        Err(_) => return Err(()),
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
pub fn init_prompt(default_config: &WSConfig) -> Result<WSConfig, ()> {
    let mut cfg = default_config.clone();

    match prompt_user() {
        Ok(v) => cfg.user = v,
        Err(_) => return Err(()),
    };

    match Confirm::new("Do you want to setup custom git repositories?")
        .with_default(false)
        .prompt()
    {
        Ok(true) => {
            match prompt_custom_git_repos(&default_config.git) {
                Ok(v) => cfg.git = v,
                Err(_) => return Err(()),
            };
        }
        Ok(false) => {}
        Err(_) => return Err(()),
    };

    match Confirm::new("Do you want to setup custom registries?")
        .with_default(false)
        .prompt()
    {
        Ok(true) => {
            match prompt_custom_registries(&default_config.registry) {
                Ok(v) => cfg.registry = v,
                Err(_) => return Err(()),
            };
        }
        Ok(false) => {}
        Err(_) => return Err(()),
    };

    Ok(cfg)
}
