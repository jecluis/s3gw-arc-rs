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

use super::Workspace;

impl Workspace {
    /// Synchronize the current workspace, showing progress bars for each
    /// individual repository in the workspace.
    ///
    pub fn sync(self: &Self) -> Result<(), ()> {
        let repos = vec![
            &self.repos.s3gw,
            &self.repos.ui,
            &self.repos.charts,
            &self.repos.ceph,
        ];

        for repo in repos {
            let bars = MultiProgress::new();
            let main = ProgressBar::new(0);
            main.set_style(
                ProgressStyle::with_template(
                    format!(
                        "{:12} [{{elapsed_precise}}] {{bar:40.cyan/blue}} {{percent}}% \
                        {{pos:>7}}/{{len:7}} {{msg}}",
                        repo.name
                    )
                    .as_str(),
                )
                .unwrap()
                .progress_chars("=> "),
            );

            let main = bars.add(main);

            let style = ProgressStyle::with_template(
                format!(
                    "{:12} [{{elapsed_precise}}] {{bar:40.cyan/blue}} {{percent}}% \
                    {{pos:>7}}/{{len:7}} {{msg}}",
                    ""
                )
                .as_str(),
            )
            .unwrap()
            .progress_chars("=> ");

            let mut indexed = ProgressBar::new(0);
            indexed.set_style(style.clone());
            let mut deltas = ProgressBar::new(0);
            deltas.set_style(style.clone());

            // indexed = bars.insert(2, indexed);
            // deltas = bars.insert(3, deltas);

            let mut last_v: u64 = 0;
            let mut last_length = 0_u64;
            let mut has_indexed = false;
            let mut has_deltas = false;

            match repo.sync(
                |phase: &str,
                 objs_recvd: u64,
                 objs_indexed: u64,
                 objs_total: u64,
                 delta_indexed: u64,
                 delta_total: u64| {
                    let total = objs_total + delta_total;
                    let n = objs_recvd + delta_indexed;

                    if objs_recvd == objs_total
                        && objs_indexed == objs_total
                        && delta_indexed == delta_total
                    {
                        return;
                    }
                    if total > last_length {
                        main.set_length(total);
                        last_length = total;
                    }
                    main.set_position(n);
                    main.set_message(format!("{}", phase));
                    last_v = objs_recvd;

                    if !has_indexed && objs_indexed > 0 {
                        indexed = bars.insert(2, indexed.clone());
                        indexed.set_length(objs_total);
                        indexed.set_message("indexing");
                        has_indexed = true;
                    }
                    if !has_deltas && delta_indexed > 0 {
                        deltas = bars.insert(3, deltas.clone());
                        deltas.set_length(delta_total);
                        deltas.set_message("applying deltas");
                        has_deltas = true;
                    }

                    if objs_indexed > 0 {
                        indexed.set_position(objs_indexed);
                    }
                    if delta_indexed > 0 {
                        deltas.set_position(delta_indexed);
                    }

                    if objs_indexed > 0 && objs_indexed == objs_total {
                        indexed.set_message("done");
                    }
                    if delta_indexed > 0 && delta_indexed == delta_total {
                        deltas.set_message("done");
                    }
                },
            ) {
                Ok(_) => {
                    main.finish_with_message("done");
                }
                Err(_) => {
                    main.finish_with_message("error");
                    return Err(());
                }
            };
        }

        Ok(())
    }
}
