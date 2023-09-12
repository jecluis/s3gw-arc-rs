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

use super::Release;

impl Release {
    pub fn status(self: &Self) {
        log::debug!("Show release status");

        if self.state.is_none() {
            println!("Release not defined");
            return;
        }

        match self.ws.sync() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error synchronizing workspace!");
                return;
            }
        };

        let state = self.state.as_ref().unwrap();
        println!("Release version: {}", state.release_version);
    }
}