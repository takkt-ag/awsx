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
use futures::stream::{FuturesOrdered, TryStreamExt};
use rusoto_core::HttpClient;
use rusoto_elbv2::{DescribeTagsInput, DescribeTargetGroupsInput, Elb, ElbClient, TagDescription};
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
        long = "load-balancer-arn",
        help = "Filter for target groups assigned to a specific load balancer"
    )]
    load_balancer_arn: Option<String>,
    #[structopt(
        long = "tags",
        help = "Filter for target groups by their tags",
        long_help = "Filter for target groups by their tags. Specify multiple `Key=Value` pairs, \
                     separated by spaces, where each key-value-pair corresponds to a tag assigned \
                     to the target groups."
    )]
    tags: Vec<Tag>,
}

pub(crate) async fn find_target_group(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let elb = ElbClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    let mut target_groups = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let mut output = elb
            .describe_target_groups(DescribeTargetGroupsInput {
                load_balancer_arn: opt.load_balancer_arn.clone(),
                marker: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        continuation_token = output.next_marker;
        if let Some(new_target_groups) = output.target_groups.as_mut() {
            target_groups.append(new_target_groups)
        }

        continuation_token.is_some()
    } {}

    let tag_descriptions: Vec<TagDescription> = target_groups
        .into_iter()
        .filter_map(|target_group| target_group.target_group_arn)
        .collect::<Vec<_>>()
        .chunks(20)
        .map(|arns| arns.to_vec())
        .map(|arns| async {
            elb.describe_tags(DescribeTagsInput {
                resource_arns: arns,
            })
            .await
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .fold(Vec::new(), |mut acc, mut tag_descriptions| {
            if let Some(tag_descriptions) = tag_descriptions.tag_descriptions.as_mut() {
                acc.append(tag_descriptions)
            }
            acc
        });
    let target_group_arn = tag_descriptions
        .into_iter()
        .filter(|tag_description| match &tag_description.tags {
            Some(resource_tags) => opt.tags.iter().all(|needle| {
                resource_tags.iter().any(|haystack| {
                    haystack.key == needle.key
                        && haystack
                            .value
                            .as_ref()
                            .map(|value| value == &needle.value)
                            .unwrap_or(false)
                })
            }),
            None => false,
        })
        .filter_map(|tag_description| tag_description.resource_arn)
        .next();

    match target_group_arn {
        Some(arn) => Ok(AwsxOutput {
            human_readable: arn.clone(),
            structured: json!({
                "success": true,
                "message": "Found target group matching given filters",
                "target_group_arn": &arn,
            }),
            successful: true,
        }),
        None => Ok(AwsxOutput {
            human_readable: "Unable to find target group matching given filters".to_owned(),
            structured: json!({
                "success": false,
                "message": "Unable to find target group matching given filters",
            }),
            successful: false,
        }),
    }
}
