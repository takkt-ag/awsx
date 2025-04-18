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
    stack::Stack,
};
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use structopt::StructOpt;

use crate::{
    util::{apply_excludes_includes, generate_deployment_metadata},
    AwsxOutput, AwsxProvider, Opt as GlobalOpt,
};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to update")]
    stack_name: String,
    #[structopt(long = "change-set-name", help = "Name for the new change set")]
    change_set_name: String,
    #[structopt(
        long = "role-arn",
        help = "IAM Role that AWS CloudFormation assumes when executing the change set"
    )]
    role_arn: Option<String>,
    #[structopt(
        short = "p",
        long = "parameter-overrides",
        help = "Parameters to override",
        long_help = "Parameters to override. Specify as multiple space-separated `Key=Value` \
                     pairs, where each key has to correspond to an existing parameter on the \
                     requested stack.\n(If you specify this parameter and --parameter-path, \
                     parameters provided here will override parameters provided via the parameter \
                     file.)"
    )]
    parameter_overrides: Vec<Parameter>,
    #[structopt(
        long = "parameter-path",
        help = "Path to a JSON parameter file",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects. The file can only contain parameters newly added to the \
                     template, unless the existing parameters are defined as \
                     `UsePreviousValue=true`.\n(If you specify this parameter and \
                     --parameter-overrides, parameters specified through --parameters will \
                     override parameters provided via the parameter file.)"
    )]
    parameter_path: Option<String>,
    #[structopt(
        long = "exclude",
        requires = "parameter-path",
        help = "Exclude parameters",
        long_help = "Exclude parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. If \
                     a parameter key matches any of the exclude-patterns, the parameter will not \
                     be applied.\n(Excludes only apply to parameters passed in via \
                     --parameter-path. Parameters provided via --parameter-overrides will not be \
                     affected by this.)"
    )]
    excludes: Vec<String>,
    #[structopt(
        long = "include",
        requires = "parameter-path",
        help = "Include parameters",
        long_help = "Include parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. \
                     Every parameter key that doesn't match any of the include-patterns will not \
                     be applied.\n(Excludes are applied before includes, and you cannot include a \
                     parameter that was previously excluded. Includes only apply to parameters \
                     passed in via --parameter-path. Parameters provided via --parameter-overrides \
                     will not be affected by this.)"
    )]
    includes: Vec<String>,
}

pub(crate) async fn override_parameters(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    // Retrieve the parameters currently set on the stack. This will return a list of parameters
    // where the previous value will be used in a change set.
    let stack = Stack::new(&opt.stack_name);
    let mut stack_parameters = stack.get_parameters_as_previous_value(&cfn).await?;

    // We now update the retrieved parameters, overriding them as specified on the command-line.
    if let Some(parameter_path) = &opt.parameter_path {
        let file = File::open(parameter_path)?;
        let reader = BufReader::new(file);
        let parameters: Parameters = {
            let parameters: Parameters = serde_json::from_reader(reader).unwrap();
            parameters
                .values()
                .filter(|parameter| !parameter.is_previous_value())
                .collect::<Vec<_>>()
                .into()
        };
        stack_parameters.update(apply_excludes_includes(
            parameters,
            &opt.excludes,
            &opt.includes,
        )?);
    }
    stack_parameters.update(&opt.parameter_overrides);

    if stack_parameters.is_empty() {
        Ok(AwsxOutput {
            human_readable: "No parameters specified (or all filtered), no change set created"
                .to_owned(),
            structured: json!({
                "success": false,
                "message": "No parameters specified (or all filtered), no change set created",
            }),
            successful: false,
        })
    } else {
        // Unless otherwise requested, we will update the deployment-metadata parameter
        if !global_opt.dont_update_deployment_metadata {
            if let Some(previous_metadata_parameter) = stack
                .get_parameter(&cfn, &global_opt.deployment_metadata_parameter)
                .await?
            {
                let metadata =
                    generate_deployment_metadata(Some(previous_metadata_parameter), None)?;
                stack_parameters.insert(
                    global_opt.deployment_metadata_parameter.clone(),
                    Parameter::WithValue {
                        key: global_opt.deployment_metadata_parameter.clone(),
                        value: metadata.to_string(),
                    },
                );
            } else {
                eprintln!(
                    "WARNING: an update to the deployment-metadata parameter '{}' was requested, \
                     but the stack does not have this parameter. The change-set will be created, \
                     although without any metadata.",
                    &global_opt.deployment_metadata_parameter,
                );
            }
        }

        stack
            .create_change_set(
                &cfn,
                &opt.change_set_name,
                opt.role_arn.as_deref(),
                &stack_parameters,
            )
            .await?;

        Ok(AwsxOutput {
            human_readable: format!(
                "Change set {} creation started successfully",
                opt.change_set_name
            ),
            structured: json!({
                "success": true,
                "message": "Change set creation started successfully",
                "change_set_name": opt.change_set_name,
            }),
            successful: true,
        })
    }
}
