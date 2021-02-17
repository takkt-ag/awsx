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
use rusoto_cloudfront::{
    CloudFront, CloudFrontClient, ListDistributionsRequest, ListTagsForResourceRequest,
};
use rusoto_core::{HttpClient, Region};
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
        help = "Filter for CloudFront distributions by their tags",
        long_help = "Filter for CloudFront distributions by their tags. Specify multiple \
                     `Key=Value` pairs, separated by spaces, where each key-value-pair corresponds \
                     to a tag assigned to the CloudFront distributions."
    )]
    tags: Vec<Tag>,
}

pub(crate) async fn find_cloudfront_distribution(
    opt: &Opt,
    _global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let cloudfront = CloudFrontClient::new_with(
        HttpClient::new()?,
        provider,
        // The region for CloudFront is hardcoded! Given that CloudFront is a global service, its
        // API is only valid within us-east-1 -- every other region returns an error.
        Region::UsEast1,
    );

    let mut cloudfront_distributions = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let output = cloudfront
            .list_distributions(ListDistributionsRequest {
                marker: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        if let Some(distribution_list) = output.distribution_list {
            continuation_token = distribution_list.next_marker;
            if let Some(mut items) = distribution_list.items {
                cloudfront_distributions.append(&mut items);
            }
        } else {
            continuation_token = None;
        }

        continuation_token.is_some()
    } {}

    let cloudfront_distribution = cloudfront_distributions
        .into_iter()
        .map(|distribution| async {
            cloudfront
                .list_tags_for_resource(ListTagsForResourceRequest {
                    resource: distribution.arn.clone(),
                })
                .await
                .map(|tags| (distribution, tags.tags.items))
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .filter_map(|(distribution, tags)| tags.map(|tags| (distribution, tags)))
        .filter(|(_, resource_tags)| {
            opt.tags.iter().all(|needle| {
                resource_tags.iter().any(|haystack| {
                    haystack.key == needle.key
                        && haystack
                            .value
                            .as_ref()
                            .map(|value| value == &needle.value)
                            .unwrap_or(false)
                })
            })
        })
        .map(|(distribution, _)| distribution)
        .next();

    match cloudfront_distribution {
        Some(cloudfront_distribution) => Ok(AwsxOutput {
            human_readable: cloudfront_distribution.id.clone(),
            structured: json!({
                "success": true,
                "message": "Found CloudFront distribution matching given filters",
                "cloudfront_distribution_arn": &cloudfront_distribution.arn,
                "cloudfront_distribution_id": &cloudfront_distribution.id,
            }),
            successful: true,
        }),
        None => Ok(AwsxOutput {
            human_readable: "Unable to find CloudFront distribution matching given filters"
                .to_owned(),
            structured: json!({
                "success": false,
                "message": "Unable to find CloudFront distribution matching given filters",
            }),
            successful: false,
        }),
    }
}
