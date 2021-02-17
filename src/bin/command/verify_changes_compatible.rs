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

use awsx::{error::Error, stack::Stack};
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::convert::TryFrom;
use structopt::StructOpt;

use crate::{
    util::{self, generate_deployment_metadata, DeploymentMetadata},
    AwsxOutput, AwsxProvider, Opt as GlobalOpt,
};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to compare against")]
    stack_name: String,
    #[structopt(
        long = "git-path",
        help = "Path to git-repository to compare against",
        long_help = "Path to git-repository to compare against. The default is to use the current \
                     working directory."
    )]
    git_path: Option<String>,
}

pub(crate) async fn verify_changes_compatible(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    // Create CloudFormation client
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider.clone(),
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    // Retrieve previous deployment metadata
    let stack = Stack::new(&opt.stack_name);
    let previous_metadata_parameter = stack
        .get_parameter(&cfn, &global_opt.deployment_metadata_parameter)
        .await?;
    let previous_metadata = previous_metadata_parameter.and_then(|previous_metadata_parameter| {
        DeploymentMetadata::try_from(previous_metadata_parameter).ok()
    });

    match previous_metadata {
        None => Ok(AwsxOutput {
            human_readable: "Stack currently deployed does not have deployment metadata"
                .to_string(),
            structured: json!({
                "success": false,
                "message": "Stack currently deployed does not have deployment metadata",
            }),
            successful: false,
        }),
        Some(previous_metadata) => {
            let git_path = opt.git_path.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .expect("Failed to get current directory")
                    .into_os_string()
                    .into_string()
                    .expect("Failed to get current directory as string")
            });
            let current_metadata = generate_deployment_metadata(None, Some(&git_path))?;
            let changes_compatible =
                util::verify_changes_compatible(&previous_metadata, &current_metadata, &git_path)?;

            if changes_compatible {
                Ok(AwsxOutput {
                    human_readable: "Changes are compatible".to_string(),
                    structured: json!({
                        "success": true,
                        "message": "Changes are compatible",
                    }),
                    successful: true,
                })
            } else {
                Ok(AwsxOutput {
                    human_readable: "Changes are NOT compatible".to_string(),
                    structured: json!({
                        "success": false,
                        "message": "Changes are NOT compatible",
                    }),
                    successful: false,
                })
            }
        }
    }
}
