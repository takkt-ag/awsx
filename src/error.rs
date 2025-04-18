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

#![allow(non_local_definitions)]

use failure::Fail;

/// Representation of an error within awsx.
#[derive(Debug, Fail)]
pub enum Error {
    /// Error caused in Rusoto, in proxy from AWS.
    #[fail(display = "failed to perform AWS action: {}", 0)]
    AwsError(#[fail(cause)] failure::Error),
    /// Error caused in git2
    #[fail(display = "failed to perform git action: {}", 0)]
    GitError(#[fail(cause)] failure::Error),
    /// The parameters provided were invalid.
    ///
    /// This can happen if either the template or stack the parameters should be applied to do not
    /// match what parameters the template or stack actually expects.
    #[fail(display = "invalid parameters provided: {}", 0)]
    InvalidParameters(String),
    /// The requested stack does not exist.
    #[fail(display = "invalid stack {}", 0)]
    InvalidStack(String),
    /// The template provided is not valid.
    #[fail(display = "invalid template: {}", 0)]
    InvalidTemplate(String),
    /// A general IO error.
    #[fail(display = "general IO error")]
    IoError(#[fail(cause)] std::io::Error),
    /// Error caused while parsing a regex
    #[fail(display = "failed to parse regex: {}", 0)]
    RegexParseError(String),
    /// General regex error cause while working with a regex
    #[fail(display = "general regex error")]
    RegexError(#[fail(cause)] failure::Error),
    /// Error caused in serde_json
    #[fail(display = "failed to perform serde_json action: {}", 0)]
    SerdeJsonError(#[fail(cause)] serde_json::Error),
    /// Deserializing the template failed.
    #[fail(display = "failed to deserialize the template")]
    TemplateDeserializationFailed(#[fail(cause)] failure::Error),
    /// TLS error caused within Rusoto.
    #[fail(display = "failed to perform Rusoto action")]
    TlsError(#[fail(cause)] failure::Error),
    /// An unknown error caused in Rusoto, in proxy from AWS.
    #[fail(display = "encountered AWS error:\n{}", 0)]
    UnknownAwsError(String),
    /// The output format specified was unknown
    #[fail(display = "specified output format is unknown: {}", 0)]
    UnknownOutputFormat(String),
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::IoError(cause)
    }
}

impl<E> From<rusoto_core::RusotoError<E>> for Error
where
    E: std::error::Error + std::marker::Send + std::marker::Sync + 'static,
{
    fn from(cause: rusoto_core::RusotoError<E>) -> Self {
        match cause {
            rusoto_core::RusotoError::Unknown(rusoto_core::request::BufferedHttpResponse {
                body,
                ..
            }) => Error::UnknownAwsError(String::from_utf8_lossy(&body).into_owned()),
            _ => Error::AwsError(cause.into()),
        }
    }
}

impl From<rusoto_core::request::TlsError> for Error {
    fn from(cause: rusoto_core::request::TlsError) -> Self {
        Error::TlsError(cause.into())
    }
}

impl From<regex::Error> for Error {
    fn from(cause: regex::Error) -> Self {
        match cause {
            regex::Error::Syntax(description) => Error::RegexParseError(description),
            _ => Error::RegexError(cause.into()),
        }
    }
}

impl From<git2::Error> for Error {
    fn from(cause: git2::Error) -> Self {
        Error::GitError(cause.into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(cause: serde_json::Error) -> Self {
        Error::SerdeJsonError(cause)
    }
}
