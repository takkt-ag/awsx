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

//! A helper for uploading content to S3.

use rusoto_core::HttpClient;
use rusoto_core::Region;
use rusoto_s3::{PutObjectRequest, S3Client, StreamingBody, S3};

use crate::{error::Error, provider::AwsxProvider};

/// A helper for uploading content to S3.
pub struct S3Uploader {
    region: Region,
    s3_client: S3Client,
}

impl S3Uploader {
    /// Create a new S3 uploader
    pub fn new(region: Region, provider: AwsxProvider) -> S3Uploader {
        let s3_client = S3Client::new_with(
            HttpClient::new().expect("Failed to create HTTP client"),
            provider,
            region.clone(),
        );
        S3Uploader { region, s3_client }
    }

    /// Upload a given body to S3.
    ///
    /// The return value is the path-like URL to the S3 object.
    pub async fn upload(
        &self,
        bucket_name: &str,
        key: &str,
        body: StreamingBody,
    ) -> Result<String, Error> {
        self.s3_client
            .put_object(PutObjectRequest {
                bucket: bucket_name.to_owned(),
                key: key.to_owned(),
                body: Some(body),
                server_side_encryption: Some("AES256".to_owned()),
                ..Default::default()
            })
            .await?;
        Ok(format!(
            "https://s3{region}.amazonaws.com/{bucket_name}/{key}",
            region = if self.region != Region::UsEast1 {
                format!("-{}", self.region.name())
            } else {
                String::new()
            },
            bucket_name = bucket_name,
            key = key,
        ))
    }
}
