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

use awsx::error::Error;
use itertools::Itertools;
use rusoto_autoscaling::{Autoscaling, AutoscalingClient, LaunchConfigurationNamesType};
use rusoto_core::HttpClient;
use rusoto_ec2::{
    DescribeInstancesRequest, DescribeLaunchTemplateVersionsRequest,
    DescribeLaunchTemplatesRequest, Ec2, Ec2Client,
};
use serde_json::json;
use std::collections::HashSet;
use structopt::StructOpt;

use crate::{AwsxOutput, AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {}

async fn amis_inuse_by_ec2(ec2: &dyn Ec2) -> Result<HashSet<String>, Error> {
    let mut instances = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let output = ec2
            .describe_instances(DescribeInstancesRequest {
                next_token: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        continuation_token = output.next_token;
        instances.extend(
            output
                .reservations
                .unwrap_or_else(|| vec![])
                .into_iter()
                .filter_map(|reservation| reservation.instances)
                .flatten(),
        );

        continuation_token.is_some()
    } {}

    let image_ids: HashSet<String> = instances
        .into_iter()
        .filter_map(|instance| instance.image_id)
        .collect();

    Ok(image_ids)
}

async fn amis_inuse_by_launchconfiguration(
    autoscaling: &dyn Autoscaling,
) -> Result<HashSet<String>, Error> {
    let mut launch_configurations = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let mut output = autoscaling
            .describe_launch_configurations(LaunchConfigurationNamesType {
                next_token: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        continuation_token = output.next_token;
        launch_configurations.append(&mut output.launch_configurations);

        continuation_token.is_some()
    } {}

    Ok(launch_configurations
        .into_iter()
        .map(|launch_configuration| launch_configuration.image_id)
        .collect())
}

async fn amis_inuse_by_launchtemplate(ec2: &dyn Ec2) -> Result<HashSet<String>, Error> {
    let mut launch_templates = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let output = ec2
            .describe_launch_templates(DescribeLaunchTemplatesRequest {
                next_token: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        continuation_token = output.next_token;
        launch_templates.extend(
            output
                .launch_templates
                .unwrap_or_else(|| vec![])
                .into_iter()
                .filter(|launch_template| launch_template.launch_template_id.is_some()),
        );

        continuation_token.is_some()
    } {}

    let mut launch_template_versions = Vec::new();
    for launch_template in launch_templates {
        continuation_token = None;
        while {
            let output = ec2
                .describe_launch_template_versions(DescribeLaunchTemplateVersionsRequest {
                    launch_template_id: launch_template.launch_template_id.clone(),
                    next_token: continuation_token.clone(),
                    ..Default::default()
                })
                .await?;
            continuation_token = output.next_token;
            launch_template_versions
                .extend(output.launch_template_versions.unwrap_or_else(|| vec![]));

            continuation_token.is_some()
        } {}
    }

    let image_ids: HashSet<String> = launch_template_versions
        .into_iter()
        .filter_map(|launch_template_version| launch_template_version.launch_template_data)
        .filter_map(|launch_template_data| launch_template_data.image_id)
        .collect();

    Ok(image_ids)
}

pub(crate) async fn find_amis_inuse(
    _opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let ec2 = Ec2Client::new_with(
        HttpClient::new()?,
        provider.clone(),
        global_opt.aws_region.clone().unwrap_or_default(),
    );
    let autoscaling = AutoscalingClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    let mut amis_inuse: HashSet<String> = HashSet::new();
    amis_inuse.extend(amis_inuse_by_ec2(&ec2).await?);
    amis_inuse.extend(amis_inuse_by_launchconfiguration(&autoscaling).await?);
    amis_inuse.extend(amis_inuse_by_launchtemplate(&ec2).await?);

    Ok(AwsxOutput {
        human_readable: format!(
            "AMI-IDs in use:\n{}",
            amis_inuse
                .clone()
                .into_iter()
                .map(|ami_id| format!("- {}", ami_id))
                .join("\n")
        ),
        structured: json!({
            "success": true,
            "amis": amis_inuse,
        }),
        successful: true,
    })
}
