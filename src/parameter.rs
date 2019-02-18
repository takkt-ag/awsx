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

//! This module enables type-safe handling of stack and template parameters.

use indexmap::IndexMap;
use serde::{de, ser};
use serde_derive::{Deserialize, Serialize};
use std::ops;
use std::str::FromStr;

/// Represents a CloudFormation stack or template parameter.
///
/// A parameter can either have a value ([`WithValue`]), or it can use the previous value
/// ([`PreviousValue`]).
///
/// [`WithValue`]: #variant.WithValue
/// [`PreviousValue`]: #variant.PreviousValue
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Parameter {
    /// Identifies a parameter with a value specified.
    WithValue {
        /// Key of the CloudFormation stack or template parameter.
        #[serde(rename = "ParameterKey")]
        key: String,
        /// Value of the CloudFormation stack or template parameter.
        #[serde(rename = "ParameterValue")]
        value: String,
    },
    /// A parameter where the previous, pre-existing value on the stack should be retained.
    PreviousValue {
        /// Key of the CloudFormation stack or template parameter.
        #[serde(rename = "ParameterKey")]
        key: String,
    },
}

impl Parameter {
    /// Create a parameter of the [`PreviousValue`] variant.
    ///
    /// This is a simple helper and is equal to instantiating the variant yourself:
    ///
    /// ```
    /// # use awsx::Parameter;
    /// assert_eq!(
    ///     Parameter::PreviousValue {
    ///         key: String::new()
    ///     },
    ///     Parameter::previous_value(String::new())
    /// );
    /// ```
    ///
    /// [`PreviousValue`]: #variant.PreviousValue
    pub fn previous_value(key: String) -> Self {
        Parameter::PreviousValue { key }
    }

    /// Convert the parameter type as returned by Rusoto CloudFormation into our Parameter type.
    ///
    /// This conversion can fail since we don't support input parameters structured as follows:
    ///
    /// * Only a `resolved_value` is present, whereas `parameter_value` isn't. The `resolved_value`
    ///   field is used in the AWS Systems Manager context, and this exact scenario probably can't
    ///   happen, but we also don't deal with it should it happen.
    ///
    /// * No `parameter_value` is given, and `use_previous_value` is not `true`.
    ///
    /// Hence we return an `Option<Parameter>`.
    pub fn from(cfn_parameter: &rusoto_cloudformation::Parameter) -> Option<Self> {
        match cfn_parameter {
            rusoto_cloudformation::Parameter {
                parameter_key: Some(ref key),
                use_previous_value: Some(true),
                ..
            } => Some(Parameter::PreviousValue {
                key: key.to_owned(),
            }),
            rusoto_cloudformation::Parameter {
                parameter_key: Some(ref key),
                parameter_value: Some(ref value),
                ..
            } => Some(Parameter::WithValue {
                key: key.to_owned(),
                value: value.to_owned(),
            }),
            _ => None,
        }
    }

    /// Convert the parameter type as returned by Rusoto CloudFormation into our Parameter type,
    /// specifically into the [`PreviousValue`] variant.
    ///
    /// This function fails should the `parameter_key` not have been present, hence it returns an
    /// `Option<Parameter>`.
    ///
    /// [`PreviousValue`]: #variant.PreviousValue
    pub fn from_as_previous_value(
        cfn_parameter: &rusoto_cloudformation::Parameter,
    ) -> Option<Self> {
        cfn_parameter
            .parameter_key
            .as_ref()
            .map(String::to_owned)
            .map(Parameter::previous_value)
    }

    /// Convert a parameter of any type into the [`PreviousValue`] variant.
    ///
    /// [`PreviousValue`]: #variant.PreviousValue
    pub fn into_previous_value(self) -> Self {
        use Parameter::*;
        Parameter::PreviousValue {
            key: match self {
                WithValue { key, .. } => key,
                PreviousValue { key, .. } => key,
            },
        }
    }

    /// Return a reference to the parameters key.
    ///
    /// This is a convenience function that abstracts matching over all variants, where `key` is a
    /// common field to all of them.
    pub fn key(&self) -> &str {
        use Parameter::*;
        match self {
            WithValue { key, .. } => &key,
            PreviousValue { key, .. } => &key,
        }
    }
}

impl From<&Parameter> for rusoto_cloudformation::Parameter {
    fn from(parameter: &Parameter) -> Self {
        use Parameter::*;
        match parameter {
            WithValue { key, value } => rusoto_cloudformation::Parameter {
                parameter_key: Some(key.to_owned()),
                parameter_value: Some(value.to_owned()),
                ..Default::default()
            },
            PreviousValue { key } => rusoto_cloudformation::Parameter {
                parameter_key: Some(key.to_owned()),
                use_previous_value: Some(true),
                ..Default::default()
            },
        }
    }
}

impl From<Parameter> for rusoto_cloudformation::Parameter {
    fn from(parameter: Parameter) -> Self {
        (&parameter).into()
    }
}

impl FromStr for Parameter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.splitn(2, '=');

        Ok(Parameter::WithValue {
            key: split
                .next()
                .ok_or_else(|| "Parameter needs to provided in the form `Key=Value`".to_owned())?
                .to_owned(),
            value: split
                .next()
                .ok_or_else(|| "Parameter needs to provided in the form `Key=Value`".to_owned())?
                .to_owned(),
        })
    }
}

/// A collection holding one or more stack or template parameters.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Parameters(IndexMap<String, Parameter>);

impl Parameters {
    /// Create the `Parameters` collection from `Vec<Parameter>`.
    pub fn new(parameters: Vec<Parameter>) -> Self {
        Parameters(
            parameters
                .into_iter()
                .map(|parameter| (parameter.key().to_owned(), parameter))
                .collect(),
        )
    }

    /// Return an iterator over the keys of the collection, in their order
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    /// Update all parameters in the current collection with the ones also available in the other
    /// collection.
    ///
    /// This can be used to override parameters, while avoiding to add new ones that the other
    /// collection might have defined.
    ///
    /// ```
    /// # use awsx::{Parameter, Parameters};
    /// let mut parameters = Parameters::new(vec![
    ///     Parameter::WithValue {
    ///         key: "FirstParameter".to_owned(),
    ///         value: "Initial value".to_owned(),
    ///     },
    ///     Parameter::PreviousValue {
    ///         key: "SecondParameter".to_owned(),
    ///     },
    ///     Parameter::PreviousValue {
    ///         key: "ThirdParameter".to_owned(),
    ///     },
    /// ]);
    /// let others = Parameters::new(vec![
    ///     Parameter::PreviousValue {
    ///         key: "FirstParameter".to_owned(),
    ///     },
    ///     Parameter::WithValue {
    ///         key: "ThirdParameter".to_owned(),
    ///         value: "New value".to_owned(),
    ///     },
    ///     Parameter::WithValue {
    ///         key: "UnknownParameter".to_owned(),
    ///         value: "New value".to_owned(),
    ///     },
    /// ]);
    /// parameters.update(others);
    ///
    /// assert_eq!(
    ///     parameters,
    ///     vec![
    ///         Parameter::PreviousValue {
    ///             key: "FirstParameter".to_owned(),
    ///         },
    ///         Parameter::PreviousValue {
    ///             key: "SecondParameter".to_owned(),
    ///         },
    ///         Parameter::WithValue {
    ///             key: "ThirdParameter".to_owned(),
    ///             value: "New value".to_owned(),
    ///         },
    ///     ].into()
    /// );
    /// ```
    pub fn update<P: IntoParameters>(&mut self, other: P) {
        for (key, value) in other.into_parameters().0 {
            if let indexmap::map::Entry::Occupied(mut entry) = self.0.entry(key) {
                entry.insert(value);
            }
        }
    }

    /// Return a new collection with all parameters in the current collection overriden by the ones
    /// also available in the other collection.
    ///
    /// In contrast to [`update`], this does not mutate the existing collection, but rather returns
    /// a new copy.
    ///
    /// [`update`]: #method.update
    pub fn updated<P: IntoParameters>(&self, other: P) -> Parameters {
        let mut this = self.clone();
        this.update(other);
        this
    }

    /// Add or update all parameters in the other collection to the current colleciton.
    ///
    /// This can be used to override parameters, but in contrast to [`update`] it will also add new
    /// parameters, which might not be what you want.
    ///
    /// ```
    /// # use awsx::{Parameter, Parameters};
    /// let mut parameters = Parameters::new(vec![
    ///     Parameter::WithValue {
    ///         key: "FirstParameter".to_owned(),
    ///         value: "Initial value".to_owned(),
    ///     },
    ///     Parameter::PreviousValue {
    ///         key: "SecondParameter".to_owned(),
    ///     },
    ///     Parameter::PreviousValue {
    ///         key: "ThirdParameter".to_owned(),
    ///     },
    /// ]);
    /// let others = Parameters::new(vec![
    ///     Parameter::PreviousValue {
    ///         key: "FirstParameter".to_owned(),
    ///     },
    ///     Parameter::WithValue {
    ///         key: "ThirdParameter".to_owned(),
    ///         value: "New value".to_owned(),
    ///     },
    ///     Parameter::WithValue {
    ///         key: "UnknownParameter".to_owned(),
    ///         value: "New value".to_owned(),
    ///     },
    /// ]);
    /// parameters.merge(others);
    ///
    /// assert_eq!(
    ///     parameters,
    ///     vec![
    ///         Parameter::PreviousValue {
    ///             key: "FirstParameter".to_owned(),
    ///         },
    ///         Parameter::PreviousValue {
    ///             key: "SecondParameter".to_owned(),
    ///         },
    ///         Parameter::WithValue {
    ///             key: "ThirdParameter".to_owned(),
    ///             value: "New value".to_owned(),
    ///         },
    ///         Parameter::WithValue {
    ///             key: "UnknownParameter".to_owned(),
    ///             value: "New value".to_owned(),
    ///         },
    ///     ].into()
    /// );
    /// ```
    ///
    /// [`update`]: #method.update
    pub fn merge<P: IntoParameters>(&mut self, other: P) {
        self.0.extend(other.into_parameters().0)
    }

    /// Return a new collection with all parameters in the current collection, adding or updating
    /// all parameters from the other collection.
    ///
    /// This can be used to override parameters, but in contrast to [`updated`] it will also add new
    /// parameters, which might not be what you want.
    ///
    /// In contrast to [`merge`], this does not mutate the existing collection, but rather returns a
    /// new copy.
    ///
    /// [`updated`]: #method.updated
    /// [`merge`]: #method.merge
    pub fn merged<P: IntoParameters>(&self, other: P) -> Parameters {
        let mut this = self.0.clone();
        this.extend(other.into_parameters().0);
        Parameters(this)
    }
}

impl From<Vec<Parameter>> for Parameters {
    fn from(parameters: Vec<Parameter>) -> Self {
        Parameters::new(parameters)
    }
}

impl From<&Vec<Parameter>> for Parameters {
    fn from(parameters: &Vec<Parameter>) -> Self {
        Parameters::new(parameters.clone())
    }
}

impl From<&Parameters> for Vec<rusoto_cloudformation::Parameter> {
    fn from(parameters: &Parameters) -> Self {
        parameters.0.iter().map(|(_, v)| v.into()).collect()
    }
}

impl From<Parameters> for Vec<rusoto_cloudformation::Parameter> {
    fn from(parameters: Parameters) -> Self {
        (&parameters).into()
    }
}

impl ser::Serialize for Parameters {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.0.values().collect::<Vec<_>>().serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for Parameters {
    fn deserialize<D>(deserializer: D) -> Result<Parameters, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(Parameters::new(Vec::deserialize(deserializer)?))
    }
}

impl ops::Sub for Parameters {
    type Output = Parameters;

    fn sub(mut self, rhs: Self) -> Self::Output {
        rhs.0.keys().for_each(|key| {
            self.0.remove(key);
        });
        self
    }
}

/// Conversion into [`Parameters`].
///
/// [`Parameters`]: struct.Parameters.html
pub trait IntoParameters {
    /// Create [`Parameters`] from a value.
    ///
    /// [`Parameters`]: struct.Parameters.html
    fn into_parameters(self) -> Parameters;
}

impl IntoParameters for Parameters {
    fn into_parameters(self) -> Parameters {
        self
    }
}

impl IntoParameters for Vec<Parameter> {
    fn into_parameters(self) -> Parameters {
        self.into()
    }
}

impl IntoParameters for &Vec<Parameter> {
    fn into_parameters(self) -> Parameters {
        self.into()
    }
}
