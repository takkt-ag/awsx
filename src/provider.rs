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

//! A Rusoto/AWS credential provider, with integrated support for role assumption.

use async_trait::async_trait;
use rusoto_core::{HttpClient, Region};
use rusoto_credential::{
    AwsCredentials, CredentialsError, DefaultCredentialsProvider, ProvideAwsCredentials,
};
use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};
use uuid::Uuid;

/// A Rusoto/AWS credential provider, with integrated support for role assumption.
///
/// The provider can be supplied with a AWS Access Key ID and AWS Secret Access Key pair to use for
/// authentication, otherwise it will check the default provider chain (environment variables,
/// instance metadata, container metadata or AWS profiles).
///
/// In addition, if a role was supplied, the provider will assume the role with the initially
/// discovered credentials, returning the new STS credentials instead.
#[derive(Clone)]
pub struct AwsxProvider {
    assume_role_arn: Option<String>,
    aws_region: Region,
    inner: AwsxInnerProvider,
}

impl AwsxProvider {
    /// Create a new AwsxProvider.
    pub fn new(
        assume_role_arn: Option<String>,
        aws_region: Region,
        aws_access_key_id: Option<String>,
        aws_secret_access_key: Option<String>,
    ) -> Result<AwsxProvider, CredentialsError> {
        Ok(AwsxProvider {
            assume_role_arn,
            aws_region,
            inner: AwsxInnerProvider::new(aws_access_key_id, aws_secret_access_key)?,
        })
    }
}

#[async_trait]
impl ProvideAwsCredentials for AwsxProvider {
    async fn credentials(&self) -> Result<AwsCredentials, CredentialsError> {
        return if let Some(assume_role_arn) = &self.assume_role_arn {
            let sts_client = StsClient::new_with(
                HttpClient::new().expect("Failed to create HTTP client"),
                self.inner.clone(),
                self.aws_region.clone(),
            );
            StsAssumeRoleSessionCredentialsProvider::new(
                sts_client,
                assume_role_arn.to_owned(),
                format!(
                    "{name}=={version}@{request_id}",
                    name = env!("CARGO_PKG_NAME"),
                    version = env!("CARGO_PKG_VERSION"),
                    request_id = Uuid::new_v4(),
                ),
                None,
                None,
                None,
                None,
            )
            .credentials()
            .await
        } else {
            self.inner.credentials().await
        };
    }
}

#[derive(Clone)]
struct AwsxInnerProvider {
    credentials: Option<AwsCredentials>,
    default_credentials_provider: DefaultCredentialsProvider,
}

impl AwsxInnerProvider {
    fn new(
        aws_access_key_id: Option<String>,
        aws_secret_access_key: Option<String>,
    ) -> Result<AwsxInnerProvider, CredentialsError> {
        let credentials = match (aws_access_key_id, aws_secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => Some(AwsCredentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
            )),
            _ => None,
        };
        Ok(AwsxInnerProvider {
            credentials,
            default_credentials_provider: DefaultCredentialsProvider::new()?,
        })
    }
}

#[async_trait]
impl ProvideAwsCredentials for AwsxInnerProvider {
    async fn credentials(&self) -> Result<AwsCredentials, CredentialsError> {
        match self.credentials {
            Some(ref credentials) => Ok(credentials.clone()),
            _ => {
                DefaultCredentialsProvider::new()
                    .unwrap()
                    .credentials()
                    .await
            }
        }
    }
}
