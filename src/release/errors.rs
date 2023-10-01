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
    CorruptedError,
    NotStartedError,
    ReleaseExistsError,
    ReleaseStartedError,
    StagingError,
    CommittingError,
    PushingError,
    SubmoduleError,
    TaggingError,
    SyncError,

    // github release build process
    ReleaseBuildOnGoingError,
    ReleaseBuildFailedError,
    ReleaseBuildNotFoundError,

    UnknownError,
}

impl Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReleaseError::AbortedError => "aborted",
            ReleaseError::CorruptedError => "corrupted release",
            ReleaseError::NotStartedError => "release not started",
            ReleaseError::ReleaseExistsError => "release already exists",
            ReleaseError::ReleaseStartedError => "release already started",
            ReleaseError::StagingError => "error staging files",
            ReleaseError::CommittingError => "error committing",
            ReleaseError::PushingError => "error pushing to remote",
            ReleaseError::SubmoduleError => "submodule error",
            ReleaseError::TaggingError => "error tagging release",
            ReleaseError::SyncError => "error synchronizing",
            // github release build process
            ReleaseError::ReleaseBuildOnGoingError => "release build in progress",
            ReleaseError::ReleaseBuildFailedError => "release build failed",
            ReleaseError::ReleaseBuildNotFoundError => "release build not found",
            // unknown error
            ReleaseError::UnknownError => "unknown error",
        })
    }
}

pub type ReleaseResult<T> = Result<T, ReleaseError>;

#[derive(Clone, Copy, Debug)]
pub enum ChartsError {
    DoesNotExistError,
    ParsingError,
    StagingError,
    CommitError,

    UnknownError,
}

impl Display for ChartsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ChartsError::DoesNotExistError => "chart file does not exist",
            ChartsError::ParsingError => "error parsing chart file",
            ChartsError::StagingError => "error staging chart file for commit",
            ChartsError::CommitError => "error committing chart file",
            ChartsError::UnknownError => "unknown error",
        })
    }
}

pub type ChartsResult<T> = Result<T, ChartsError>;
