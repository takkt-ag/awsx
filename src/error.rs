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

//! Errors within awsx

use failure::Fail;

/// Representation of an error within awsx.
#[derive(Debug, Fail)]
pub enum Error {
    /// Error caused by Rusoto, in proxy from AWS.
    #[fail(display = "failed to perform AWS action")]
    AwsError(#[fail(cause)] failure::Error),
    /// The parameters provided were invalid.
    ///
    /// This can happen if either the template or stack the parameters should be applied to do not
    /// match what parameters the template or stack actually expects.
    #[fail(display = "invalid parameters provided: {}", 0)]
    InvalidParameters(String),
    /// The requested stack does not exist.
    #[fail(display = "invalid stack {}", 0)]
    InvalidStack(String),
    /// A general IO error.
    #[fail(display = "general IO error")]
    IoError(#[fail(cause)] std::io::Error),
    /// Error caused within Rusoto.
    #[fail(display = "failed to perform Rusoto action")]
    RusotoError(#[fail(cause)] failure::Error),
    /// Deserializing the template failed.
    #[fail(display = "failed to deserialize the template")]
    TemplateDeserializationFailed(#[fail(cause)] failure::Error),
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::IoError(cause)
    }
}

impl From<rusoto_cloudformation::CreateChangeSetError> for Error {
    fn from(cause: rusoto_cloudformation::CreateChangeSetError) -> Self {
        Error::AwsError(cause.into())
    }
}

impl From<rusoto_cloudformation::DescribeStacksError> for Error {
    fn from(cause: rusoto_cloudformation::DescribeStacksError) -> Self {
        Error::AwsError(cause.into())
    }
}

impl From<rusoto_s3::PutObjectError> for Error {
    fn from(cause: rusoto_s3::PutObjectError) -> Self {
        Error::AwsError(cause.into())
    }
}

impl From<rusoto_core::request::TlsError> for Error {
    fn from(cause: rusoto_core::request::TlsError) -> Self {
        Error::RusotoError(cause.into())
    }
}
