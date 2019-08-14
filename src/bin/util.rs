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
use git2::{Config, Repository};
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

    metadata.user = Config::open_default()?.get_string("user.email")?;
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

        metadata.user = repo.config()?.get_string("user.email")?;
        metadata.git = DeploymentMetadataGit {
            commit,
            r#ref,
            dirty,
        }
    }

    Ok(metadata)
}
