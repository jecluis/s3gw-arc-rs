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

use std::fmt::Display;

#[derive(Clone, Copy, Debug)]
pub enum ReleaseError {
    AbortedError,
    AlreadyInit,
    CorruptedError,
    InitError,
    NotStartedError,
    ReleaseExistsError,
    UserAbortedError,

    UnknownError,
}

impl Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReleaseError::AbortedError => "aborted",
            ReleaseError::AlreadyInit => "release already init",
            ReleaseError::CorruptedError => "corrupted release",
            ReleaseError::InitError => "unable to init release",
            ReleaseError::NotStartedError => "release not started",
            ReleaseError::ReleaseExistsError => "release already exists",
            ReleaseError::UserAbortedError => "user aborted",
            ReleaseError::UnknownError => "unknown error",
        })
    }
}
