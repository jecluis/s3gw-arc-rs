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

use indicatif::{ProgressBar, ProgressStyle};

use super::{config::WSConfig, repository::Repos};

pub struct WSState {}

pub struct Workspace {
    path: PathBuf,
    config: WSConfig,
    state: Option<WSState>,
    repos: Repos,
}

impl Workspace {
    pub fn open(path: &PathBuf) -> Result<Workspace, ()> {
        let arcpath = path.join(".arc");
        let cfgpath = arcpath.join("config.json");
        let statepath = arcpath.join("state.json");

        if !arcpath.exists() || !cfgpath.exists() {
            log::error!("Workspace at {} does not exist!", path.display());
            return Err(());
        }

        let cfg = match WSConfig::read(&cfgpath) {
            Ok(v) => v,
            Err(_) => {
                log::error!("Unable to open workspace config at {}", cfgpath.display());
                return Err(());
            }
        };

        let repos = match Repos::init(&path, &cfg.user, &cfg.git) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };

        let ws = Workspace {
            path: path.to_path_buf(),
            config: cfg,
            state: None,
            repos,
        };
        match ws.sync() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error while synchronizing workspace");
                return Err(());
            }
        };
        Ok(ws)
    }

    pub fn sync(self: &Self) -> Result<(), ()> {
        let repos = vec![
            &self.repos.s3gw,
            &self.repos.ui,
            &self.repos.charts,
            &self.repos.ceph,
        ];

        for repo in repos {
            let bar = ProgressBar::new(0);
            bar.set_style(
                ProgressStyle::with_template(
                    format!(
                        "{:12} [{{elapsed_precise}}] {{bar:40.cyan/blue}} {{percent}}% {{pos:>7}}/{{len:7}} {{msg}}",
                        repo.name
                    )
                    .as_str(),
                )
                .unwrap()
                .progress_chars("=> "),
            );
            let mut last_v: u64 = 0;
            let mut has_length = false;
            match repo.sync(|phase: &str, n: u64, total: u64| {
                if n == last_v {
                    return;
                }
                if !has_length && total > 0 {
                    bar.set_length(total);
                    has_length = true;
                }
                bar.set_position(n);
                bar.set_message(format!("{}", phase));
                last_v = n;
            }) {
                Ok(_) => {
                    bar.finish_with_message("done");
                }
                Err(_) => {
                    bar.finish_with_message("error");
                    return Err(());
                }
            };
        }

        Ok(())
    }
}
