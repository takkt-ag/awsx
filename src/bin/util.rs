// Copyright 2025 TAKKT Industrial & Packaging GmbH
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
//
// SPDX-License-Identifier: Apache-2.0

use awsx::{
    error::Error,
    parameter::{Parameter, Parameters},
};
use chrono::{Local, SecondsFormat};
use failure::format_err;
use git2::{Config, Oid, Repository};
use regex::RegexSet;
use serde_derive::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct DeploymentMetadata {
    pub(crate) user: String,
    pub(crate) when: String,
    pub(crate) git: DeploymentMetadataGit,
}

impl fmt::Display for DeploymentMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct DeploymentMetadataGit {
    pub(crate) commit: String,
    pub(crate) r#ref: String,
    pub(crate) dirty: bool,
}

impl TryFrom<Parameter> for DeploymentMetadata {
    type Error = Error;

    fn try_from(parameter: Parameter) -> Result<Self, Self::Error> {
        match parameter {
            Parameter::WithValue { value, .. } => serde_json::from_str(&value).map_err(Into::into),
            Parameter::PreviousValue { key } => Err(Error::InvalidParameters(key)),
        }
    }
}

pub(crate) fn apply_excludes_includes(
    mut parameters: Parameters,
    excludes: &[String],
    includes: &[String],
) -> Result<Parameters, Error> {
    if !excludes.is_empty() {
        let excludes = RegexSet::new(excludes)?;
        parameters = parameters
            .values()
            .filter(|parameter| !excludes.is_match(parameter.key()))
            .collect::<Vec<_>>()
            .into()
    }
    if !includes.is_empty() {
        let includes = RegexSet::new(includes)?;
        parameters = parameters
            .values()
            .filter(|parameter| includes.is_match(parameter.key()))
            .collect::<Vec<_>>()
            .into()
    }

    Ok(parameters)
}

pub(crate) fn generate_deployment_metadata(
    previous_metadata_parameter: Option<Parameter>,
    git_discover_path: Option<&str>,
) -> Result<DeploymentMetadata, Error> {
    let mut metadata = previous_metadata_parameter
        .and_then(|parameter| DeploymentMetadata::try_from(parameter).ok())
        .unwrap_or_default();

    metadata.user = Config::open_default()?
        .get_string("user.email")
        .unwrap_or_else(|_| "unknown".to_owned());
    metadata.when = Local::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    if let Some(git_discover_path) = git_discover_path {
        let repo = Repository::discover(git_discover_path)?;
        let head = repo.head()?;
        let r#ref = head
            .shorthand()
            .ok_or_else(|| Error::GitError(format_err!("Failed to retrieve ref for git HEAD")))?
            .to_owned();
        let commit = format!(
            "{}",
            head.target().ok_or_else(|| Error::GitError(format_err!(
                "Failed to retrieve commit for git HEAD"
            )))?
        );
        let statuses = repo.statuses(Some(git2::StatusOptions::new().include_untracked(false)))?;
        let dirty = !statuses.is_empty();

        metadata.user = repo
            .config()?
            .get_string("user.email")
            .unwrap_or_else(|_| "unknown".to_owned());
        metadata.git = DeploymentMetadataGit {
            commit,
            r#ref,
            dirty,
        }
    }

    Ok(metadata)
}

pub(crate) fn verify_changes_compatible(
    previous_metadata: &DeploymentMetadata,
    current_metadata: &DeploymentMetadata,
    git_discover_path: &str,
) -> Result<bool, Error> {
    // Find the common ancestor
    let previous_commit_is_common_ancestor =
        if previous_metadata.git.commit == current_metadata.git.commit {
            true
        } else {
            // Retrieve previous and current commit
            let repo = Repository::discover(git_discover_path)?;
            let previous_commit = Oid::from_str(&previous_metadata.git.commit)?;
            let current_commit = repo.head()?.target().ok_or_else(|| {
                Error::GitError(format_err!("Failed to retrieve commit for git HEAD"))
            })?;

            match repo.merge_base(previous_commit, current_commit) {
                Ok(common_ancestor) => previous_commit == common_ancestor,
                Err(ref e)
                    if e.code() == git2::ErrorCode::GenericError
                        && e.class() == git2::ErrorClass::Odb =>
                {
                    // If either of the commits we are comparing is unknown to the repository, the
                    // error returned will be of code `GenericError` and class `Odb` (bad object).
                    // Rather than showing that error, which can commonly occur if either the
                    // deployed changes are based on a commit another developer only has locally, or
                    // if the user has rebased their own changes since the last time they deployed,
                    // we simply return `false` here indicating that the changes are not compatible.
                    false
                }
                Err(e) => return Err(e.into()),
            }
        };

    // In general it is true that if the previous changes were dirty, we cannot guarantee any
    // compatibility. We make one exception: if the user stays unchanged, and the previous commit is
    // the common ancestor, we assume that the change is just the person developing and testing.
    if previous_metadata.git.dirty {
        return Ok(
            previous_metadata.user == current_metadata.user && previous_commit_is_common_ancestor
        );
    }

    // If the previous changes were not dirty, we can now verify if the current commit is a direct
    // descendant from the previous commit. If it isn't, the two commits are out of two separate
    // trees and we thus cannot assume them to be compatible.
    Ok(previous_commit_is_common_ancestor)
}
