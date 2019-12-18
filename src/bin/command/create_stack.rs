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
    s3::S3Uploader,
    template::Template,
};
use itertools::Itertools;
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::{fs::File, io::BufReader};
use structopt::StructOpt;

use crate::{util::generate_deployment_metadata, AwsxOutput, AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to create")]
    stack_name: String,
    #[structopt(long = "change-set-name", help = "Name for the new change set")]
    change_set_name: String,
    #[structopt(
        long = "role-arn",
        help = "IAM Role that AWS CloudFormation assumes when executing the change set"
    )]
    role_arn: Option<String>,
    #[structopt(long = "template-path", help = "Path to the new template")]
    template_path: String,
    #[structopt(
        short = "p",
        long = "parameters",
        help = "Parameters required by template",
        long_help = "Parameters required by template. Specify as multiple `Key=Value` pairs, where \
                     each key has to correspond to a parameter newly added to the template, i.e. \
                     the parameter can not be already defined on the stack.\n(If you specify this \
                     parameter and --parameter-path, parameters provided here will override \
                     parameters provided via the parameter file.)"
    )]
    parameters: Vec<Parameter>,
    #[structopt(
        long = "parameter-path",
        help = "Path to a JSON parameter file",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects. The file shuold contain all parameters required by the \
                     template (parameters with defaults can be skipped).\n(If you specify this \
                     parameter and --parameter-overrides, parameters specified through \
                     --parameters will override parameters provided via the parameter file.)"
    )]
    parameter_path: Option<String>,
    #[structopt(
        long = "force-create",
        help = "Force change set creation",
        long_help = "Force change set creation, even if the parameters supplied do not cover all \
                     required parameters exactly, or if the stack you are trying to deploy. This \
                     means that if you force change set creation, the created change set might \
                     be invalid."
    )]
    force_create: bool,
}

pub(crate) fn create_stack(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    // Load the template
    let template = Template::new(&opt.template_path)?;

    // Create AWS clients
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider.clone(),
        global_opt.aws_region.clone().unwrap_or_default(),
    );
    let s3 = S3Uploader::new(global_opt.aws_region.clone().unwrap_or_default(), provider);
    let s3_upload: Option<(&S3Uploader, &str)> = global_opt
        .s3_bucket_name
        .as_ref()
        .map(|bucket_name| (&s3, bucket_name.as_ref()));

    // Retrieve the parameters defined on the template.
    let mut template_parameters = template.get_parameters().to_owned();

    // Get the user provided parameters.
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
        template_parameters.update(parameters);
    }
    template_parameters.update(&opt.parameters);

    // Unless otherwise requested, we will set the deployment-metadata parameter
    if !global_opt.dont_update_deployment_metadata {
        let metadata = generate_deployment_metadata(None, Some(&opt.template_path))?;
        template_parameters.insert(
            global_opt.deployment_metadata_parameter.clone(),
            Parameter::WithValue {
                key: global_opt.deployment_metadata_parameter.clone(),
                value: metadata.to_string(),
            },
        );
    }

    // We need to ensure that the user has provided all parameters required by the template.
    let missing_parameters = template_parameters
        .iter()
        .filter_map(|(name, parameter)| {
            if parameter.is_previous_value() {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !missing_parameters.is_empty() {
        if opt.force_create {
            eprintln!(
                "WARNING: some required parameters ({}) have not been supplied. The change set \
                 will be created since it was explicitly requested!",
                missing_parameters.iter().join(", ")
            );
        } else {
            return Err(Error::InvalidParameters(format!(
                "not all required parameters ({}) were provided",
                missing_parameters.iter().join(", ")
            )));
        }
    }

    // Create the change set for the new template, including the new parameters.
    template.create_change_set(
        &cfn,
        &opt.change_set_name,
        &opt.stack_name,
        &template_parameters,
        opt.role_arn.as_ref().map(|role_arn| &**role_arn),
        s3_upload,
        true,
    )?;

    Ok(AwsxOutput {
        human_readable: format!(
            "Change set {} creation for new stack {} started successfully",
            opt.change_set_name, opt.stack_name,
        ),
        structured: json!({
            "success": true,
            "stack_name": opt.stack_name,
            "change_set_name": opt.change_set_name,
        }),
        successful: true,
    })
}
