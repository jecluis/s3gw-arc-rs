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
mod git;
mod ws;

#[tokio::main]
async fn main() {
    env_logger::init();
    let cmd = args::parse();

    match &cmd.command {
        args::Command::WS(cmd) => match &cmd.command {
            args::WSCmds::Init(init) => {
                log::info!("path: {}", init.path.display());

                let ws = match ws::init::init(&init.path) {
                    Ok(v) => {
                        log::info!("Success!");
                        v
                    }
                    Err(_) => {
                        log::error!("Error!");
                        return;
                    }
                };
            }
            args::WSCmds::Info => {
                log::info!("info");
            }
        },
    };
}
