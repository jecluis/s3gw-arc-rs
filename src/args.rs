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

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Workspace related actions.
    WS(WorkspaceCommand),
    /// Release related actions.
    Rel(ReleaseCommand),
}

#[derive(Args)]
#[command()]
pub struct WorkspaceCommand {
    #[command(subcommand)]
    pub command: crate::ws::cmds::Cmds,
}

#[derive(Args)]
#[command()]
pub struct ReleaseCommand {
    #[command(subcommand)]
    pub command: crate::release::cmds::Cmds,
}

pub fn parse() -> Cli {
    Cli::parse()
}
