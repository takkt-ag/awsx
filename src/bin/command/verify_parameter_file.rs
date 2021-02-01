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
    template::Template,
};
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde::Serialize;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use structopt::StructOpt;

use crate::{util::apply_defaults, AwsxOutput, AwsxProvider, Opt as GlobalOpt};

const NO_ECHO_PARAMETER_VALUE: &str = "****";

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(
        long = "stack-name",
        required_unless = "template-path",
        conflicts_with = "template-path",
        help = "Name of the stack",
        long_help = "Name of the stack to compare the parameter-file against. You cannot specify \
                     this if --template-path has been specified."
    )]
    stack_name: Option<String>,
    #[structopt(
        long = "template-path",
        required_unless = "stack-name",
        conflicts_with = "stack-name",
        help = "Path to the template",
        long_help = "Path to the template-file to compare the parameter-file against. You cannot \
                     specify this if --stack-name has been specified."
    )]
    template_path: Option<String>,
    #[structopt(
        long = "parameter-path",
        required = true,
        help = "Path to a JSON parameter file",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects."
    )]
    parameter_path: String,
    #[structopt(
        long = "parameter-defaults-path",
        help = "Path to a JSON parameter file with defaults",
        long_help = "Path to a JSON parameter file, from which values will be taken if not \
                     specified in the regular parameter file. This file should be structured the \
                     same as the AWS CLI expects. If the provided path does not exist, no error is \
                     thrown, instead it will be simply ignored."
    )]
    parameter_defaults_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct UnequalParameterDifference<'a> {
    stack: &'a Parameter,
    template: &'a Parameter,
}

impl<'a> UnequalParameterDifference<'a> {
    fn from(unequal_parameters: Vec<(&'a Parameter, &'a Parameter)>) -> Vec<Self> {
        // By convention, the "left" parameter in our case is the stack, whereas the "right"
        // parameter is the used parameter-file.
        unequal_parameters
            .into_iter()
            .filter(|(stack, ..)| match stack {
                // If the stack's parameter value is equal to the "magic" value for `NoEcho`
                // parameters, we don't want to include it in the unequal parameter list, since we
                // can't compare a value we have to a `NoEcho` parameter.
                Parameter::WithValue { value, .. } if value == NO_ECHO_PARAMETER_VALUE => false,
                _ => true,
            })
            .map(|(stack, template)| UnequalParameterDifference { stack, template })
            .collect()
    }
}

pub(crate) async fn verify_parameter_file(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    // Load parameter file
    let file = File::open(&opt.parameter_path)?;
    let reader = BufReader::new(file);
    let mut file_parameters: Parameters = serde_json::from_reader(reader).unwrap();

    // Apply defaults if provided
    file_parameters = apply_defaults(file_parameters, &opt.parameter_defaults_path)?;

    let defined_parameters = if let Some(stack_name) = &opt.stack_name {
        // Create AWS clients
        let cfn = CloudFormationClient::new_with(
            HttpClient::new()?,
            provider.clone(),
            global_opt.aws_region.clone().unwrap_or_default(),
        );
        // Retrieve stack parameters
        let stack = Stack::new(stack_name);
        stack.get_parameters(&cfn).await?
    } else if let Some(template_path) = &opt.template_path {
        // Load the template
        let template = Template::new(template_path)?;
        // Retrieve the parameters defined on the template.
        template.get_parameters().to_owned()
    } else {
        // clap should catch this situation before this code-path is ever reached.
        unreachable!();
    };

    // Compare
    let differences = defined_parameters.loose_difference(&file_parameters);
    if let Some(differences) = differences {
        let mut table = prettytable::Table::new();
        table.set_format(*prettytable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(row![
            "Only on stack or template",
            "Unequal between both",
            "Only in parameter file",
        ]);
        for parameter in &differences.left {
            table.add_row(row![parameter.key(), "", ""]);
        }
        for parameter in &differences.unequal {
            table.add_row(row!["", parameter.0.key(), ""]);
        }
        for parameter in &differences.right {
            table.add_row(row!["", "", parameter.key()]);
        }

        let mut human_readable = Vec::new();
        table.print(&mut human_readable)?;
        let human_readable =
            String::from_utf8(human_readable).expect("prettytable did not return UTF-8");

        Ok(AwsxOutput {
            human_readable,
            structured: json!({
                "success": false,
                "parameters": {
                    "only_on_stack_or_template": differences.left,
                    "equal_between_both": differences.equal,
                    "unequal_between_both": UnequalParameterDifference::from(differences.unequal),
                    "only_in_parameter_file": differences.right,
                },
            }),
            successful: false,
        })
    } else {
        Ok(AwsxOutput {
            human_readable: "The parameters in the given file MATCH the CloudFormation stack."
                .to_string(),
            structured: json!({
                "success": true,
                "parameters": {
                    "only_on_stack_or_template": [],
                    "equal_between_both": defined_parameters,
                    "unequal_between_both": [],
                    "only_in_parameter_file": [],
                },
            }),
            successful: true,
        })
    }
}
