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

#![deny(missing_docs)]
#![deny(bare_trait_objects)]

//! # awsx
//!
//! This library can be seen as an AWS CLI extension, providing various types and helpers that are
//! mainly used to allow the confident automation of CloudFormation deployments.
//!
//! This specifically is the library used internally in the `awsx` binary. For further documentation
//! on how to use the binary, please check the respective documentation.

pub mod error;
pub mod parameter;
pub mod provider;
pub mod s3;
pub mod stack;
pub mod template;
