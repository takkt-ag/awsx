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

use std::str::FromStr;

use futures::stream::{self, StreamExt};
use rusoto_core::HttpClient;
use rusoto_rds::{
    DBClusterSnapshot, DescribeDBClusterSnapshotsMessage, ListTagsForResourceMessage, Rds,
    RdsClient,
};
use serde_json::json;
use structopt::StructOpt;

use awsx::error::Error;

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
        long = "db-cluster-identifier",
        help = "Filter for DB snapshots assigned from a specific DB cluster"
    )]
    db_cluster_identifier: Option<String>,
    #[structopt(long = "snapshot-type", help = "Filter DB snapshots by their type")]
    snapshot_type: Option<String>,
    #[structopt(
        long = "tags",
        help = "Filter for target groups by their tags",
        long_help = "Filter for DB snapshots by their tags. Specify multiple `Key=Value` pairs, \
                     separated by spaces, where each key-value-pair corresponds to a tag assigned \
                     to the DB snapshot."
    )]
    tags: Vec<Tag>,
}

pub(crate) async fn find_db_cluster_snapshot(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    let rds = RdsClient::new_with(
        HttpClient::new()?,
        provider,
        global_opt.aws_region.clone().unwrap_or_default(),
    );

    let mut db_cluster_snapshots = Vec::new();
    let mut continuation_token: Option<String> = None;
    while {
        let mut output = rds
            .describe_db_cluster_snapshots(DescribeDBClusterSnapshotsMessage {
                db_cluster_identifier: opt.db_cluster_identifier.clone(),
                snapshot_type: opt.snapshot_type.clone(),
                marker: continuation_token.clone(),
                ..Default::default()
            })
            .await?;
        continuation_token = output.marker;
        if let Some(new_db_cluster_snapshots) = output.db_cluster_snapshots.as_mut() {
            db_cluster_snapshots.append(new_db_cluster_snapshots)
        }

        continuation_token.is_some()
    } {}

    let enriched_db_cluster_snapshots: Vec<(DBClusterSnapshot, Vec<rusoto_rds::Tag>)> =
        stream::iter(db_cluster_snapshots.into_iter())
            .filter_map(|db_cluster_snapshot| async {
                if let Some(db_cluster_snapshot_arn) = &db_cluster_snapshot.db_cluster_snapshot_arn
                {
                    let tags = rds
                        .list_tags_for_resource(ListTagsForResourceMessage {
                            resource_name: db_cluster_snapshot_arn.clone(),
                            ..Default::default()
                        })
                        .await;
                    if let Ok(tags) = tags {
                        tags.tag_list
                            .map(|tag_list| (db_cluster_snapshot, tag_list))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .await;

    let db_cluster_snapshot_arn = enriched_db_cluster_snapshots
        .into_iter()
        .filter(|(_, tag_list)| {
            opt.tags.iter().all(|needle| {
                tag_list.iter().any(|haystack| {
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
            })
        })
        .map(|(db_cluster_snapshot, _)| db_cluster_snapshot)
        .max_by_key(|db_cluster_snapshot| db_cluster_snapshot.snapshot_create_time.clone())
        .and_then(|db_cluster_snapshot| db_cluster_snapshot.db_cluster_snapshot_arn);

    match db_cluster_snapshot_arn {
        Some(db_cluster_snapshot_arn) => Ok(AwsxOutput {
            human_readable: db_cluster_snapshot_arn.clone(),
            structured: json!({
                "success": true,
                "message": "Found DB cluster-snapshot matching given filters",
                "db_cluster_snapshot_arn": &db_cluster_snapshot_arn,
            }),
            successful: true,
        }),
        None => Ok(AwsxOutput {
            human_readable: "Unable to find DB cluster-snapshot matching given filters".to_owned(),
            structured: json!({
                "success": false,
                "message": "Unable to find DB cluster-snapshot matching given filters",
            }),
            successful: false,
        }),
    }
}
