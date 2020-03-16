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

use awsx::{error::Error, parameter::Parameters, stack::Stack};
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use structopt::StructOpt;

use crate::{AwsxOutput, AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack")]
    stack_name: String,
    #[structopt(
        long = "parameter-path",
        help = "Path to a JSON parameter file",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects."
    )]
    parameter_path: String,
}

pub(crate) async fn verify_parameter_file(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    // Load parameter file
    let file = File::open(&opt.parameter_path)?;
    let reader = BufReader::new(file);
    let file_parameters: Parameters = serde_json::from_reader(reader).unwrap();

    // Create AWS clients
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider.clone(),
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    // Retrieve stack parameters
    let stack = Stack::new(&opt.stack_name);
    let stack_parameters = stack.get_parameters(&cfn).await?;

    // Compare
    let differences = stack_parameters.loose_difference(&file_parameters);
    if let Some(differences) = differences {
        let mut table = prettytable::Table::new();
        table.set_format(*prettytable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(row![
            "Only in stack",
            "Unequal between both",
            "Only in template"
        ]);
        for parameter in &differences.left {
            table.add_row(row![parameter.key(), "", ""]);
        }
        for parameter in &differences.unequal {
            table.add_row(row!["", parameter.key(), ""]);
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
                    "only_on_stack": differences.left,
                    "unequal_between_both": differences.unequal,
                    "only_in_template": differences.right,
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
                    "only_on_stack": [],
                    "unequal_between_both": [],
                    "only_in_template": [],
                },
            }),
            successful: true,
        })
    }
}
