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

use futures::{
    future::{err, ok, Either},
    Future, Poll,
};
use rusoto_core::{
    credential::{AwsCredentials, ChainProvider, ProvideAwsCredentials},
    CredentialsError, HttpClient, Region,
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
#[derive(Debug, Clone)]
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
    ) -> AwsxProvider {
        AwsxProvider {
            assume_role_arn,
            aws_region,
            inner: AwsxInnerProvider::new(aws_access_key_id, aws_secret_access_key),
        }
    }
}

impl ProvideAwsCredentials for AwsxProvider {
    type Future = AwsxProviderFuture;

    fn credentials(&self) -> Self::Future {
        let future = if let Some(assume_role_arn) = &self.assume_role_arn {
            let sts_client = StsClient::new_with(
                HttpClient::new().expect("Failed to create HTTP client"),
                self.inner.clone(),
                self.aws_region.clone(),
            );
            Either::A(
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
                .credentials(),
            )
        } else {
            Either::B(self.inner.credentials())
        };

        AwsxProviderFuture {
            inner: Box::new(future),
        }
    }
}

/// The inner future used to drive the credential request to completion.
pub struct AwsxProviderFuture {
    inner: Box<dyn Future<Item = AwsCredentials, Error = CredentialsError> + Send>,
}

impl Future for AwsxProviderFuture {
    type Item = AwsCredentials;
    type Error = CredentialsError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}

#[derive(Debug, Clone)]
struct AwsxInnerProvider {
    credentials: Option<AwsCredentials>,
    chain_provider: ChainProvider,
}

impl AwsxInnerProvider {
    fn new(
        aws_access_key_id: Option<String>,
        aws_secret_access_key: Option<String>,
    ) -> AwsxInnerProvider {
        let credentials = match (aws_access_key_id, aws_secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => Some(AwsCredentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
            )),
            _ => None,
        };
        AwsxInnerProvider {
            credentials,
            chain_provider: ChainProvider::new(),
        }
    }
}

impl ProvideAwsCredentials for AwsxInnerProvider {
    type Future = AwsxProviderFuture;

    fn credentials(&self) -> Self::Future {
        let chain_provider = self.chain_provider.clone();
        let future = match self.credentials {
            Some(ref credentials) => ok(credentials.clone()),
            None => err(CredentialsError::new("")),
        }
        .or_else({ move |_| chain_provider.credentials() });
        AwsxProviderFuture {
            inner: Box::new(future),
        }
    }
}
