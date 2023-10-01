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
pub enum RepositoryError {
    UnableToOpenRepositoryError,
    UnableToGetReferencesError,
    UnknownBranchError,
    UnknownSHA1Error,
    SubmoduleHeadUpdateError,
    StagingError,

    // git related errors
    FetchingError,
    PushingError,
    CheckoutError,
    RemoteUpdateError,
    SubmoduleUpdateError,
    BranchingError,

    UnknownError,
}

impl Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RepositoryError::UnableToOpenRepositoryError => "unable to open repository",
            RepositoryError::UnableToGetReferencesError => "unable to obtain git references",
            RepositoryError::UnknownBranchError => "unknown branch",
            RepositoryError::UnknownSHA1Error => "unknown SHA1",
            RepositoryError::SubmoduleHeadUpdateError => "error updating submodule HEAD",
            RepositoryError::StagingError => "error staging paths",

            // git related errors
            RepositoryError::FetchingError => "error fetching from remote",
            RepositoryError::PushingError => "error pushing to remote",
            RepositoryError::CheckoutError => "error checking out branch",
            RepositoryError::RemoteUpdateError => "error updating remote",
            RepositoryError::SubmoduleUpdateError => "error updating submodules",
            RepositoryError::BranchingError => "error branching",

            // unknown error
            RepositoryError::UnknownError => "unknown error",
        })
    }
}

pub type RepositoryResult<T> = Result<T, RepositoryError>;
