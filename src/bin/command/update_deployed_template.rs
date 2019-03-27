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
    stack::Stack,
    template::Template,
};
use itertools::Itertools;
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use structopt::StructOpt;

use crate::{util::apply_excludes_includes, AwsxOutput, AwsxProvider, Opt as GlobalOpt};

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
    #[structopt(long = "template-path", help = "Path to the new template")]
    template_path: String,
    #[structopt(
        short = "p",
        long = "parameters",
        conflicts_with = "parameter_path",
        help = "New parameters required by template",
        long_help = "New parameters required by template. Specify as multiple `Key=Value` pairs, \
                     where each key has to correspond to a parameter newly added to the template, \
                     i.e. the parameter can not be already defined on the stack.\n(If you specify \
                     this parameter, you cannot specify --parameter-path, --exclude or --include.)"
    )]
    parameters: Vec<Parameter>,
    #[structopt(
        long = "parameter-path",
        help = "Path to a JSON parameter file",
        conflicts_with = "parameters",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects. The file can only contain parameters newly added to the \
                     template, unless the existing parameters are defined as \
                     `UsePreviousValue=true`.\n(If you specify this parameter, you cannot specify \
                     --parameters.)"
    )]
    parameter_path: Option<String>,
    #[structopt(
        long = "exclude",
        conflicts_with = "parameters",
        requires = "parameter_path",
        help = "Exclude parameters",
        long_help = "Exclude parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. If \
                     a parameter key matches any of the exclude-patterns, the parameter will not \
                     be applied."
    )]
    excludes: Vec<String>,
    #[structopt(
        long = "include",
        conflicts_with = "parameters",
        requires = "parameter_path",
        help = "Include parameters",
        long_help = "Include parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. \
                     Every parameter key that doesn't match any of the include-patterns will not \
                     be applied.\n(Excludes are applied before includes, and you cannot include a \
                     parameter that was previously excluded.)"
    )]
    includes: Vec<String>,
}

pub(crate) fn update_stack(
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

    let stack = Stack::new(&opt.stack_name);

    // Retrieve the parameters defined on the template, as well as the current parameters defined on
    // the stack.
    let mut template_parameters = template.get_parameters().to_owned();
    let stack_parameters = stack.get_parameters_as_previous_value(&cfn)?;

    // Identify newly added parameters, which are parameters defined on the template, but not on the
    // stack. (Parameters that are defined on the stack but not on the template, so the other way
    // around, are simply ignored, since they do not need to be set and will simply be removed
    // once the change-set is deployed.)
    let new_parameters = template_parameters.clone() - stack_parameters;

    // Get the user provided parameters.
    let provided_parameters: Parameters = if let Some(parameter_path) = &opt.parameter_path {
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
        apply_excludes_includes(parameters, &opt.excludes, &opt.includes)?
    } else {
        (&opt.parameters).into()
    };

    // We need to ensure that the user has provided exactly the parameters that have been added.
    if !new_parameters
        .keys()
        .sorted()
        .eq(provided_parameters.keys().sorted())
    {
        return Err(Error::InvalidParameters(
            "all newly required parameters have to be provided, and no old or non-existant \
             parameters can be specified"
                .to_owned(),
        ));
    }

    // Update the template parameters with the provided parameters.
    template_parameters.update(provided_parameters);

    // Create the change set for the new template, including the new parameters.
    template.create_change_set(
        &cfn,
        &opt.change_set_name,
        &opt.stack_name,
        &template_parameters,
        opt.role_arn.as_ref().map(|role_arn| &**role_arn),
        s3_upload,
    )?;

    Ok(AwsxOutput {
        human_readable: format!(
            "Change set {} creation started successfully",
            opt.change_set_name
        ),
        structured: json!({
            "success": true,
            "change_set_name": opt.change_set_name,
        }),
        successful: true,
    })
}
