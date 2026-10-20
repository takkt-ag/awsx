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

//! This module enables handling of CloudFormation stack tags.

use rusoto_cloudformation::Tag as CfnTag;
use std::str::FromStr;

/// A stack tag, parsed from a `Key=Value` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The key of the tag.
    pub key: String,
    /// The value of the tag.
    pub value: String,
}

impl FromStr for Tag {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('=') {
            Some((key, value)) if !key.is_empty() => Ok(Tag {
                key: key.to_owned(),
                value: value.to_owned(),
            }),
            _ => Err("Tag needs to be provided in the form `Key=Value`".to_owned()),
        }
    }
}

impl From<Tag> for CfnTag {
    fn from(tag: Tag) -> Self {
        CfnTag {
            key: tag.key,
            value: tag.value,
        }
    }
}

/// Merge the `new` tags over the `existing` ones; on key collisions the new value wins.
///
/// CloudFormation *replaces* the whole tag set of a stack when tags are provided, so the existing
/// tags have to be carried over explicitly.
pub fn merge_tags(existing: Vec<CfnTag>, new: &[Tag]) -> Vec<CfnTag> {
    let mut merged: Vec<CfnTag> = existing
        .into_iter()
        .filter(|existing| !new.iter().any(|new| new.key == existing.key))
        .collect();
    merged.extend(new.iter().cloned().map(Into::into));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value() {
        assert_eq!(
            "Key=a=b".parse::<Tag>(),
            Ok(Tag {
                key: "Key".to_owned(),
                value: "a=b".to_owned()
            })
        );
        assert!("Key".parse::<Tag>().is_err());
        assert!("=value".parse::<Tag>().is_err());
    }

    #[test]
    fn new_tags_override_and_existing_are_kept() {
        let existing = vec![
            CfnTag {
                key: "Keep".to_owned(),
                value: "1".to_owned(),
            },
            CfnTag {
                key: "Override".to_owned(),
                value: "old".to_owned(),
            },
        ];
        let new = vec!["Override=new".parse().unwrap(), "Added=2".parse().unwrap()];
        let merged = merge_tags(existing, &new);
        let pairs: Vec<_> = merged.iter().map(|t| (&*t.key, &*t.value)).collect();
        assert_eq!(
            pairs,
            vec![("Keep", "1"), ("Override", "new"), ("Added", "2")]
        );
    }
}
