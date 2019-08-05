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
use rusoto_autoscaling::{AutoScalingGroupNamesType, Autoscaling, AutoscalingClient};
use rusoto_core::HttpClient;
use serde_json::json;
use std::str::FromStr;
use structopt::StructOpt;

use crate::{AwsxOutput, AwsxProvider, Opt as GlobalOpt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tag {
    key: String,
    value: String,
}

impl FromStr for Tag {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.splitn(2, '=');

        Ok(Tag {
            key: split
                .next()
                .ok_or_else(|| "Tag needs to be provided in the form `Key=Value`".to_owned())?
                .to_owned(),
            value: split
                .next()
                .ok_or_else(|| "Tag needs to be provided in the form `Key=Value`".to_owned())?
                .to_owned(),
        })
    }
}

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(
        long = "tags",
        required = true,
        help = "Filter for auto-scaling groups by their tags",
        long_help = "Filter for auto-scaling groups by their tags. Specify multiple `Key=Value` \
                     pairs, separated by spaces, where each key-value-pair corresponds to a tag \
                     assigned to the auto-scaling groups."
    )]
    tags: Vec<Tag>,
}

pub(crate) fn find_auto_scaling_group(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let autoscaling = AutoscalingClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    let mut auto_scaling_groups = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let mut output = autoscaling
            .describe_auto_scaling_groups(AutoScalingGroupNamesType {
                next_token: continuation_token.clone(),
                ..Default::default()
            })
            .sync()?;
        continuation_token = output.next_token;
        auto_scaling_groups.append(&mut output.auto_scaling_groups);

        continuation_token.is_some()
    } {}

    let auto_scaling_group =
        auto_scaling_groups
            .into_iter()
            .find(|auto_scaling_group| match &auto_scaling_group.tags {
                Some(resource_tags) => opt.tags.iter().all(|needle| {
                    resource_tags.iter().any(|haystack| {
                        haystack
                            .key
                            .as_ref()
                            .map(|key| key == &needle.key)
                            .unwrap_or(false)
                            && haystack
                                .value
                                .as_ref()
                                .map(|value| value == &needle.value)
                                .unwrap_or(false)
                    })
                }),
                None => false,
            });

    match auto_scaling_group {
        Some(auto_scaling_group) => Ok(AwsxOutput {
            human_readable: auto_scaling_group.auto_scaling_group_name.clone(),
            structured: json!({
                "success": true,
                "message": "Found auto-scaling group matching given filters",
                "auto_scaling_group_arn":  auto_scaling_group
                    .auto_scaling_group_arn
                    .map(serde_json::Value::String)
                    .unwrap_or_else(|| serde_json::Value::Null),
                "auto_scaling_group_name": &auto_scaling_group.auto_scaling_group_name,
            }),
            successful: true,
        }),
        None => Ok(AwsxOutput {
            human_readable: "Unable to find auto-scaling group matching given filters".to_owned(),
            structured: json!({
                "success": false,
                "message": "Unable to find auto-scaling group matching given filters",
            }),
            successful: false,
        }),
    }
}
