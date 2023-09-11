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

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

#[macro_export]
macro_rules! warnln {
    ($msg:expr) => {{
        extern crate colored;
        use colored::*;
        println!("\u{26A0}\u{fe0f}  {}", $msg.magenta().bold());
        log::warn!("{}", $msg);
    }};
}

#[macro_export]
macro_rules! infoln {
    ($msg:expr) => {{
        extern crate colored;
        use colored::*;
        println!("\u{2139}\u{fe0f}  {}", $msg.cyan().bold());
        log::info!("{}", $msg);
    }};
}

#[macro_export]
macro_rules! boomln {
    ($msg:expr) => {{
        extern crate colored;
        use colored::*;
        println!("\u{1f4a5} {}", $msg.red().bold());
        log::error!("{}", $msg);
    }};
}

#[macro_export]
macro_rules! errorln {
    ($msg:expr) => {{
        extern crate colored;
        use colored::*;
        println!("\u{274c}  {}", $msg.red().bold());
        log::error!("{}", $msg);
    }};
}

#[macro_export]
macro_rules! successln {
    ($msg:expr) => {
        extern crate colored;
        use colored::*;
        println!("\u{2705} {}", $msg.green().bold());
        log::info!("{}", $msg);
    };
}

pub struct RepoSyncProgress {
    #[allow(dead_code)]
    name: String,
    bars: MultiProgress,

    main_bar: ProgressBar,
    indexes_bar: ProgressBar,
    deltas_bar: ProgressBar,

    last_length: u64,
    has_indexes: bool,
    has_deltas: bool,
}

pub struct RepoUpdateProgress {
    progress: ProgressBar,
}

impl RepoSyncProgress {
    pub fn new(name: &String) -> RepoSyncProgress {
        let prefix_len = 12.max(name.len());
        let main_bar = ProgressBar::new(0);
        let indexes_bar = ProgressBar::new(0);
        let deltas_bar = ProgressBar::new(0);
        main_bar.set_style(RepoSyncProgress::get_bar_style(&prefix_len, &name));
        indexes_bar.set_style(RepoSyncProgress::get_bar_style(&prefix_len, &"".into()));
        deltas_bar.set_style(RepoSyncProgress::get_bar_style(&prefix_len, &"".into()));

        let bars = MultiProgress::new();
        let main_bar = bars.add(main_bar);

        RepoSyncProgress {
            name: name.clone(),
            bars,
            main_bar,
            indexes_bar,
            deltas_bar,
            last_length: 0,
            has_indexes: false,
            has_deltas: false,
        }
    }

    fn get_bar_style(prefix_len: &usize, name: &String) -> ProgressStyle {
        ProgressStyle::with_template(
            format!(
                "{:len$} [{{elapsed_precise}}] {{bar:40.cyan/blue}} {{percent}}% \
                {{pos:>9}}/{{len:9}} {{msg}}",
                name,
                len = prefix_len
            )
            .as_str(),
        )
        .unwrap()
        .progress_chars("=> ")
    }

    pub fn handle_values(
        self: &mut Self,
        what: &str,
        objs_recvd: u64,
        objs_indexed: u64,
        objs_total: u64,
        delta_indexed: u64,
        delta_total: u64,
    ) {
        let total = objs_total + delta_total;
        let n = objs_recvd + delta_indexed;

        if total > self.last_length {
            self.main_bar.set_length(total);
            self.last_length = total;
        }
        self.main_bar.set_position(n);
        self.main_bar.set_message(format!("{}", what));

        if !self.has_indexes && objs_indexed > 0 {
            self.indexes_bar = self.bars.insert(2, self.indexes_bar.clone());
            self.indexes_bar.set_length(objs_total);
            self.indexes_bar.set_message("indexing");
            self.has_indexes = true;
        }
        if !self.has_deltas && delta_indexed > 0 {
            self.deltas_bar = self.bars.insert(3, self.deltas_bar.clone());
            self.deltas_bar.set_length(delta_total);
            self.deltas_bar.set_message("applying deltas");
            self.has_deltas = true;
        }

        if objs_indexed > 0 {
            self.indexes_bar.set_position(objs_indexed);
        }
        if delta_indexed > 0 {
            self.deltas_bar.set_position(delta_indexed);
        }

        if objs_indexed > 0 && objs_indexed == objs_total {
            self.indexes_bar.set_message("done");
        }
        if delta_indexed > 0 && delta_indexed == delta_total {
            self.deltas_bar.set_message("done");
        }

        if objs_recvd == objs_total && objs_indexed == objs_total && delta_indexed == delta_total {
            return;
        }
    }

    pub fn finish(self: &mut Self) {
        self.main_bar.set_message("done");
    }

    pub fn finish_with_error(self: &mut Self) {
        self.main_bar.set_message("error");
    }
}

impl RepoUpdateProgress {
    pub fn new(name: &String) -> RepoUpdateProgress {
        let len = 12.max(name.len());
        let progress = ProgressBar::new_spinner();
        progress.enable_steady_tick(std::time::Duration::from_millis(200));
        progress.set_style(
            ProgressStyle::with_template(
                format!(
                    "{{spinner:.dim.bold}} {{prefix:{}.bold}}: update {{msg}}",
                    len
                )
                .as_str(),
            )
            .unwrap()
            .tick_strings(&[" ⣼", " ⣹", " ⢻", " ⠿", " ⡟", " ⣏", " ⣧", " ⣶", "✅"]),
        );
        progress.set_prefix(name.clone());

        RepoUpdateProgress { progress }
    }

    pub fn start(self: &Self) {
        self.progress.tick();
    }

    pub fn set_message(self: &Self, msg: &String) {
        self.progress.set_message(msg.clone());
    }

    pub fn finish(self: &Self) {
        self.progress.finish_with_message("done");
    }

    pub fn finish_with_error(self: &Self) {
        self.progress.finish_with_message("error");
    }
}
