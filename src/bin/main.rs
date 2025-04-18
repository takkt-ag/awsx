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

use awsx::{error::Error, provider::AwsxProvider};
use rusoto_core::Region;
use serde::{Serialize, Serializer};
use std::str::FromStr;
use structopt::StructOpt;

mod command;
mod util;

use command::{
    create_stack, find_amis_inuse, find_auto_scaling_group, find_cloudfront_distribution,
    find_db_cluster_snapshot, find_db_snapshot, find_target_group, identify_new_parameters,
    override_parameters, update_deployed_template, verify_changes_compatible,
    verify_parameter_file,
};

#[derive(Debug, StructOpt)]
#[structopt(author)]
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
        requires = "aws-secret-access-key"
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
        requires = "aws-access-key-id"
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
    #[structopt(
        long = "output-format",
        help = "Specify the format of the application output",
        long_help = "Specify the format of the application output. The default, if left \
                     unspecified, depends on whether stdout is a TTY. If it is, the output will be \
                     human readable. If it isn't, the contents will be output in structured form, \
                     specifically JSON.",
        possible_values(&["human", "human-readable", "structured", "json", "yml", "yaml"])
    )]
    pub output_format: Option<OutputFormat>,
    #[structopt(
        long = "s3-bucket-name",
        help = "Name of the S3 bucket used for storing templates",
        long_help = "Name of the S3 bucket used for storing templates. Any command that updates a \
                     stack template will upload the template to S3 if this parameter is specified. \
                     If the parameter is unspecified, the awsx will try to provide the template \
                     within the API call to AWS, although the template size here is limited to \
                     51,200 bytes (enforced by the AWS API)."
    )]
    pub s3_bucket_name: Option<String>,
    #[structopt(
        long = "dont-update-deployment-metadata",
        help = "Do not update the stack parameter containing deployment metadata",
        long_help = "Unless specified, awsx will automatically update a stack-parameter containing \
                     deployment metadata with the latest information for commands that update a \
                     stack. If you specify this option, awsx will not update the deployment \
                     metadata."
    )]
    pub dont_update_deployment_metadata: bool,
    #[structopt(
        long = "deployment-metadata-parameter",
        default_value = "DeploymentMetadata",
        help = "Parameter in which deployment metadata will be stored",
        long_help = "Parameter of the stack in which deployment metadata will be stored. This \
                     applies to all commands that update a stack. You can opt-out of metadata \
                     updates with the `--dont-update-deployment-metadata` option."
    )]
    pub deployment_metadata_parameter: String,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(
        name = "create-stack",
        author,
        about = "Create a new stack with given parameters",
        after_help = "IAM permissions required:\n\
                      - cloudformation:DescribeStacks\n\
                      - cloudformation:CreateChangeSet\n\
                      - s3:PutObject"
    )]
    CreateStack(create_stack::Opt),
    #[structopt(
        name = "find-amis-inuse",
        author,
        about = "Identify all AMI-IDs that are being used",
        long_about = "Identify all AMI-IDs that are being used within a region and account. For \
                      this the command analyzes all AWS resources where AMI-IDs can be referenced, \
                      and returns a complete list of the AMI-IDs in-use.",
        after_help = "IAM permissions required:\n\
                      - ec2:DescribeInstances\n\
                      - ec2:DescribeLaunchTemplates\n\
                      - ec2:DescribeLaunchTemplateVersions\n\
                      - autoscaling:DescribeLaunchConfigurations"
    )]
    FindAmisInuse(find_amis_inuse::Opt),
    #[structopt(
        name = "find-auto-scaling-group",
        author,
        about = "Find an auto scaling group based on its tags",
        after_help = "IAM permissions required:\n\
                      - autoscaling:DescribeAutoScalingGroups"
    )]
    FindAutoScalingGroup(find_auto_scaling_group::Opt),
    #[structopt(
        name = "find-cloudfront-distribution",
        author,
        about = "Find a CloudFront distribution based on its tags",
        after_help = "IAM permissions required:\n\
                      - cloudfront:ListDistributions\n\
                      - cloudfront:ListTagsForResource"
    )]
    FindCloudfrontDistribution(find_cloudfront_distribution::Opt),
    #[structopt(
        name = "find-db-cluster-snapshot",
        author,
        about = "Find a DB cluster snapshot based on its tags",
        after_help = "IAM permissions required:\n\
                      - rds:DescribeDBClusterSnapshots\n\
                      - rds:ListTagsForResource"
    )]
    FindDBClusterSnapshot(find_db_cluster_snapshot::Opt),
    #[structopt(
        name = "find-db-snapshot",
        author,
        about = "Find a DB snapshot based on its tags",
        after_help = "IAM permissions required:\n\
                      - rds:DescribeDBSnapshots\n\
                      - rds:ListTagsForResource"
    )]
    FindDBSnapshot(find_db_snapshot::Opt),
    #[structopt(
        name = "find-target-group",
        author,
        about = "Find a target group based on its tags",
        after_help = "IAM permissions required:\n\
                      - elasticloadbalancing:DescribeTargetGroups\n\
                      - elasticloadbalancing:DescribeTags"
    )]
    FindTargetGroup(find_target_group::Opt),
    #[structopt(
        name = "identify-new-parameters",
        author,
        about = "Show new template parameters not present on the stack",
        long_about = "Show all new parameters defined on the template, but not present on the \
                      stack. This subcommand does not create a change set, and performs only \
                      read-only actions.",
        after_help = "IAM permissions required:\n\
                      - cloudformation:DescribeStacks"
    )]
    IdentifyNewParameters(identify_new_parameters::Opt),
    #[structopt(
        name = "override-parameters",
        author,
        about = "Update specified parameters on an existing stack",
        long_about = "Update specified parameters on an existing stack, without updating the \
                      underlying template. Only the specified parameters will be updated, with all \
                      other parameters staying unchanged. NOTE: this will only create a change set \
                      that will not be automatically executed.",
        after_help = "IAM permissions required:\n\
                      - cloudformation:DescribeStacks\n\
                      - cloudformation:CreateChangeSet"
    )]
    OverrideParameters(override_parameters::Opt),
    #[structopt(
        name = "update-deployed-template",
        author,
        about = "Update an existing stack with a new template",
        long_about = "Update an existing stack with a new template, without updating any \
                      parameters already defined on the stack. You can and have to supply \
                      parameters that are newly added. NOTE: this will only create a change set \
                      that will not be automatically executed.",
        after_help = "IAM permissions required:\n\
                      - cloudformation:DescribeStacks\n\
                      - cloudformation:CreateChangeSet\n\
                      - s3:PutObject"
    )]
    UpdateDeployedTemplate(update_deployed_template::Opt),
    #[structopt(
        name = "verify-changes-compatible",
        author,
        about = "Verify that the deployed and local changes are compatible",
        long_about = "Verify that the deployed stack is compatible with the local changes.",
        after_help = "IAM permissions requried:\n\
                      - cloudformation:DescribeStacks"
    )]
    VerifyChangesCompatible(verify_changes_compatible::Opt),
    #[structopt(
        name = "verify-parameter-file",
        author,
        about = "Verify that parameters in a file match a deployed stack",
        long_about = "Verify that the parameters defined in your parameters file match a currently \
                      deployed stack. If your parameter-file has parameters defined as \
                      `UsePreviousValue`, they will be considered equal to whatever is defined on \
                      the stack. This subcommand does not create a change set, and performs only \
                      read-only actions.",
        after_help = "IAM permissions required:\n\
                      - cloudformation:DescribeStacks"
    )]
    VerifyParameterFile(verify_parameter_file::Opt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputFormat {
    HumanReadable,
    Json,
    Yaml,
}

impl Default for OutputFormat {
    fn default() -> Self {
        if atty::is(atty::Stream::Stdout) {
            OutputFormat::HumanReadable
        } else {
            OutputFormat::Json
        }
    }
}

impl FromStr for OutputFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "human" | "human-readable" => Ok(OutputFormat::HumanReadable),
            "json" | "structured" => Ok(OutputFormat::Json),
            "yml" | "yaml" => Ok(OutputFormat::Yaml),
            _ => Err(Error::UnknownOutputFormat(s.to_owned())),
        }
    }
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    let provider = AwsxProvider::new(
        opt.assume_role_arn.clone(),
        opt.aws_region.clone().unwrap_or_default(),
        opt.aws_access_key_id.clone(),
        opt.aws_secret_access_key.clone(),
    )
    .expect("failed to create awsx credential provider");

    use Command::*;
    let output: Result<AwsxOutput, Error> = match opt.command {
        CreateStack(ref command_opt) => {
            create_stack::create_stack(command_opt, &opt, provider).await
        }
        FindAmisInuse(ref command_opt) => {
            find_amis_inuse::find_amis_inuse(command_opt, &opt, provider).await
        }
        FindAutoScalingGroup(ref command_opt) => {
            find_auto_scaling_group::find_auto_scaling_group(command_opt, &opt, provider).await
        }
        FindCloudfrontDistribution(ref command_opt) => {
            find_cloudfront_distribution::find_cloudfront_distribution(command_opt, &opt, provider)
                .await
        }
        FindDBClusterSnapshot(ref command_opt) => {
            find_db_cluster_snapshot::find_db_cluster_snapshot(command_opt, &opt, provider).await
        }
        FindDBSnapshot(ref command_opt) => {
            find_db_snapshot::find_db_snapshot(command_opt, &opt, provider).await
        }
        FindTargetGroup(ref command_opt) => {
            find_target_group::find_target_group(command_opt, &opt, provider).await
        }
        IdentifyNewParameters(ref command_opt) => {
            identify_new_parameters::identify_new_parameters(command_opt, &opt, provider).await
        }
        OverrideParameters(ref command_opt) => {
            override_parameters::override_parameters(command_opt, &opt, provider).await
        }
        UpdateDeployedTemplate(ref command_opt) => {
            update_deployed_template::update_stack(command_opt, &opt, provider).await
        }
        VerifyChangesCompatible(ref command_opt) => {
            verify_changes_compatible::verify_changes_compatible(command_opt, &opt, provider).await
        }
        VerifyParameterFile(ref command_opt) => {
            verify_parameter_file::verify_parameter_file(command_opt, &opt, provider).await
        }
    };
    match output {
        Ok(output) => {
            let output_string = match opt.output_format.unwrap_or_default() {
                OutputFormat::HumanReadable => output.human_readable,
                OutputFormat::Json => serde_json::to_string(&output.structured).unwrap(),
                OutputFormat::Yaml => serde_yaml::to_string(&output.structured).unwrap(),
            };
            if output.successful {
                println!("{}", output_string);
            } else {
                eprintln!("{}", output_string);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
}

#[derive(Debug)]
pub(crate) struct AwsxOutput {
    human_readable: String,
    structured: serde_json::Value,
    successful: bool,
}

impl Serialize for AwsxOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.structured.serialize(serializer)
    }
}
