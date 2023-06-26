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

use super::{config::WSConfig, prompt::init_prompt, workspace::Workspace};

pub fn init(path: &PathBuf) -> Result<Workspace, ()> {
    let arcpath = path.join(".arc");
    let cfgpath = arcpath.join("config.json");

    if !path.exists() || !arcpath.exists() || !cfgpath.exists() {
        match create_workspace(path) {
            Ok(_) => {}
            Err(_) => {
                log::error!("Unable to create workspace at {}", path.display());
                return Err(());
            }
        };
    }

    Workspace::open(path)
}

fn create_workspace(path: &PathBuf) -> Result<(), ()> {
    let arcpath = path.join(".arc");
    if !arcpath.exists() {
        std::fs::create_dir_all(&arcpath).expect("Unable to create directories");
    }

    assert!(arcpath.is_dir());
    let cfgpath = arcpath.join("config.json");
    assert!(!cfgpath.exists());

    let cfg = match init_prompt(&WSConfig::default()) {
        Ok(v) => v,
        Err(_) => {
            log::error!("Unable to generate workspace config");
            return Err(());
        }
    };
    match cfg.write(&cfgpath) {
        Ok(_) => {}
        Err(_) => {
            log::error!("Unable to write workspace config at {}", cfgpath.display());
            return Err(());
        }
    };
    log::info!("Wrote workspace config at {}", cfgpath.display());

    Ok(())
}
