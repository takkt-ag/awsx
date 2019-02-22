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

use awsx::{error::Error, stack::Stack, template::Template};
use itertools::Itertools;
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use structopt::StructOpt;

use crate::{AwsxOutput, AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to update")]
    stack_name: String,
    #[structopt(long = "template-path", help = "Path to the new template")]
    template_path: String,
}

pub(crate) fn identify_new_parameters(
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

    let stack = Stack::new(&opt.stack_name);

    // Retrieve the parameters defined on the template, as well as the current parameters defined on
    // the stack.
    let template_parameters = template.get_parameters().to_owned();
    let stack_parameters = stack.get_parameters_as_previous_value(&cfn)?;

    // Identify newly added parameters, which are parameters defined on the template, but not on the
    // stack. (Parameters that are defined on the stack but not on the template, so the other way
    // around, are simply ignored, since they do not need to be set and will simply be removed
    // once the change-set is deployed.)
    let new_parameters = template_parameters.clone() - stack_parameters;

    let human_readable = {
        let parameters = new_parameters
            .keys()
            .map(|key| format!("- {}", key))
            .join("\n");
        format!(
            "New parameters defined in the template, not set on the stack:\n{}",
            parameters
        )
    };
    let structured = json!({
        "success": true,
        "parameters": new_parameters.keys().collect::<Vec<_>>(),
    });

    Ok(AwsxOutput {
        human_readable,
        structured,
    })
}
