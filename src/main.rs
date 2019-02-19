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

use awsx::provider::AwsxProvider;
use rusoto_core::Region;
use structopt::StructOpt;

mod command;
use command::override_parameters;

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(
        long = "aws-region",
        help = "Region the AWS API calls should be performed in",
        long_help = "Region the AWS API calls should be performed in. If left unspecified, the \
                     region will be determined automatically, falling back to us-east-1 should it \
                     fail."
    )]
    pub aws_region: Option<Region>,
    #[structopt(
        long = "aws-access-key-id",
        help = "AWS Access Key ID used for AWS API authentication",
        long_help = "AWS Access Key ID to use when authenticating against the AWS API. If left \
                     unspecified, the default credential provider will be used to determine the \
                     credentials (via environment variables, instance metadata, container metadata \
                     or AWS profiles). You have to specify --aws-secret-access-key too if you \
                     specify this parameter.",
        requires = "aws_secret_access_key"
    )]
    pub aws_access_key_id: Option<String>,
    #[structopt(
        long = "aws-secret-access-key",
        help = "AWS Secret Access Key used for AWS API authentication",
        long_help = "AWS Secret Access Key to use when authenticating against the AWS API. If left \
                     unspecified, the default credential provider will be used to determine the \
                     credentials (via environment variables, instance metadata, container metadata \
                     or AWS profiles). You have to specify --aws-access-key-id too if you specify \
                     this parameter.",
        requires = "aws_access_key_id"
    )]
    pub aws_secret_access_key: Option<String>,
    #[structopt(
        long = "assume-role-arn",
        help = "Optional role to assume before executing AWS API calls",
        long_help = "Optional role to assume before executing AWS API calls. This can be used to \
                     execute commands in other accounts, or to separate the actions performable \
                     in a single account. If unspecified, no role will be assumed."
    )]
    pub assume_role_arn: Option<String>,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(
        name = "override-parameters",
        about = "Update specified parameters on an existing stack",
        long_about = "Update specified parameters on an existing stack, without updating the \
                      underlying template. Only the specified parameters will be updated, with all \
                      other parameters staying unchanged. NOTE: this will only create a change set \
                      that will not be automatically executed."
    )]
    OverrideParameters(override_parameters::Opt),
}

fn main() {
    let opt = Opt::from_args();
    println!("{:#?}", opt);
    let provider = AwsxProvider::new(
        opt.assume_role_arn.clone(),
        opt.aws_region.clone().unwrap_or_default(),
        opt.aws_access_key_id.clone(),
        opt.aws_secret_access_key.clone(),
    );

    use Command::*;
    if let Err(e) = match opt.command {
        OverrideParameters(ref command_opt) => {
            override_parameters::override_parameters(command_opt, &opt, provider)
        }
    } {
        eprintln!("{:#?}", e);
    };
}
