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

mod args;
mod common;
mod git;
mod release;
mod ws;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .parse_env("ARC_DEBUG")
        .try_init()
        .unwrap();
    let cmd = args::parse();

    match &cmd.command {
        args::Command::WS(cmd) => ws::cmds::handle_cmds(&cmd.command),
        args::Command::Rel(cmd) => release::cmds::handle_cmds(&cmd.command),
    };
}
