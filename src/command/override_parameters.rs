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

use awsx::{error::Error, parameter::Parameter, stack::Stack};
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use structopt::StructOpt;

use crate::{AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to update")]
    stack_name: String,
    #[structopt(long = "change-set-name", help = "Name for the new change set")]
    change_set_name: String,
    #[structopt(
        short = "p",
        long = "parameter-overrides",
        help = "Parameters to override",
        long_help = "Parameters to override. Specify as multiple `Key=Value` pairs, where each key \
                     has to correspond to an existing parameter on the requested stack."
    )]
    parameter_overrides: Vec<Parameter>,
}

pub(crate) fn override_parameters(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<(), Error> {
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    // Retrieve the parameters currently set on the stack. This will return a list of parameters
    // where the previous value will be used in a change set.
    let stack = Stack::new(&opt.stack_name);
    let mut stack_parameters = stack.get_parameters(&cfn)?;

    // We now update the retrieved parameters, overriding them as specified on the command-line.
    stack_parameters.update(&opt.parameter_overrides);
    println!("{:#?}", stack_parameters);

    // We now create a change set for the stack, re-using the existing template.
    stack.create_change_set(&cfn, &opt.change_set_name, &stack_parameters)?;

    Ok(())
}
