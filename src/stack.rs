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

//! This module enables handling of CloudFormation stacks.

use rusoto_cloudformation::{CloudFormation, CreateChangeSetInput, CreateChangeSetOutput};

use crate::{
    error::Error,
    parameter::{Parameter, Parameters},
};

/// Represents a CloudFormation stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    name: String,
}

impl Stack {
    /// Create a new representation of a CloudFormation stack.
    pub fn new<S: AsRef<str>>(name: S) -> Stack {
        Stack {
            name: name.as_ref().to_owned(),
        }
    }

    /// Get the value of a single parameter of the stack.
    ///
    /// *Note:* internally this retrieves all parameters defined on the stack.
    pub fn get_parameter(
        &self,
        cfn: &CloudFormation,
        key: &str,
    ) -> Result<Option<Parameter>, Error> {
        Ok(self.get_parameters(cfn)?.get(key).cloned())
    }

    /// Get the current parameters for the stack.
    ///
    /// This retrieves all parameters defined on the AWS CloudFormation stack, including their
    /// current values.
    pub fn get_parameters(&self, cfn: &CloudFormation) -> Result<Parameters, Error> {
        let response = cfn
            .describe_stacks(rusoto_cloudformation::DescribeStacksInput {
                stack_name: Some(self.name.clone()),
                ..Default::default()
            })
            .sync()?;
        let stack = response
            .stacks
            .and_then(|stacks| stacks.get(0).cloned())
            .ok_or_else(|| Error::InvalidStack(self.name.clone()))?;

        Ok(stack
            .parameters
            .as_ref()
            .map(|parameters| {
                parameters
                    .iter()
                    .filter_map(|parameter| Parameter::from(parameter))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![])
            .into())
    }

    /// Get the current parameters for the stack, as previous values.
    ///
    /// This retrieves all parameters defined on the AWS CloudFormation stack and turns them into
    /// the `Parameter::PreviousValue` variant, i.e. dropping all values.
    pub fn get_parameters_as_previous_value(
        &self,
        cfn: &CloudFormation,
    ) -> Result<Parameters, Error> {
        Ok(self
            .get_parameters(cfn)?
            .0
            .into_iter()
            .map(|(_, parameter)| parameter.into_previous_value())
            .collect::<Vec<_>>()
            .into())
    }

    /// Create a change set for the current stack with the provided parameters.
    ///
    /// # Notes
    ///
    /// * This will not verify that the parameters provided are valid parameters for the stack in
    ///   question.
    ///
    /// * This method will not wait for the change set creation to complete. This means that the
    ///   creation can fail although this method returned a successful output.
    ///
    ///   Waiting for the change set should be performed externally through the AWS CLI, using the
    ///   `aws cloudformation wait change-set-create-complete` command.
    pub fn create_change_set(
        &self,
        cfn: &CloudFormation,
        name: &str,
        role_arn: Option<&str>,
        parameters: &Parameters,
    ) -> Result<CreateChangeSetOutput, Error> {
        cfn.create_change_set(CreateChangeSetInput {
            stack_name: self.name.clone(),
            use_previous_template: Some(true),
            change_set_name: name.to_owned(),
            capabilities: Some(vec![
                "CAPABILITY_IAM".to_owned(),
                "CAPABILITY_NAMED_IAM".to_owned(),
                "CAPABILITY_AUTO_EXPAND".to_owned(),
            ]),
            change_set_type: Some("UPDATE".to_owned()),
            role_arn: role_arn.map(ToOwned::to_owned),
            parameters: Some(parameters.into()),
            ..Default::default()
        })
        .sync()
        .map_err(Into::into)
    }
}
