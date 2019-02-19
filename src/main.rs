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

use futures::{
    future::{err, ok, Either},
    Future, Poll,
};
use rusoto_core::{
    credential::{AwsCredentials, ChainProvider, ProvideAwsCredentials},
    CredentialsError, HttpClient, Region,
};
use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};
use structopt::StructOpt;
use uuid::Uuid;

#[derive(Debug, StructOpt)]
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
        requires = "aws_secret_access_key"
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
        requires = "aws_access_key_id"
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
}

fn main() {
    let opt = Opt::from_args();
    println!("{:#?}", opt);
    let provider = AwsxProvider::new(&opt);
}

struct AwsxProvider {
    assume_role_arn: Option<String>,
    aws_region: Region,
    inner: AwsxInnerProvider,
}

impl AwsxProvider {
    fn new(opt: &Opt) -> AwsxProvider {
        AwsxProvider {
            assume_role_arn: opt.assume_role_arn.clone(),
            aws_region: opt.aws_region.clone().unwrap_or_default(),
            inner: AwsxInnerProvider::new(opt),
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

struct AwsxProviderFuture {
    inner: Box<Future<Item = AwsCredentials, Error = CredentialsError> + Send>,
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
    opt_provider: Option<AwsCredentials>,
    chain_provider: ChainProvider,
}

impl AwsxInnerProvider {
    fn new(opt: &Opt) -> AwsxInnerProvider {
        let opt_provider = match (&opt.aws_access_key_id, &opt.aws_secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => Some(AwsCredentials::new(
                access_key_id.as_ref(),
                secret_access_key.as_ref(),
                None,
                None,
            )),
            _ => None,
        };
        AwsxInnerProvider {
            opt_provider,
            chain_provider: ChainProvider::new(),
        }
    }
}

impl ProvideAwsCredentials for AwsxInnerProvider {
    type Future = AwsxProviderFuture;

    fn credentials(&self) -> Self::Future {
        let chain_provider = self.chain_provider.clone();
        let future = match self.opt_provider {
            Some(ref credentials) => ok(credentials.clone()),
            None => err(CredentialsError::new("")),
        }
        .or_else({ move |_| chain_provider.credentials() });
        AwsxProviderFuture {
            inner: Box::new(future),
        }
    }
}
