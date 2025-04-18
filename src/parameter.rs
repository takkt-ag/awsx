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
use itertools::Itertools;
use serde::{de, ser, Deserialize, Serialize};
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
    #[serde(serialize_with = "serialize_parameter_previousvalue")]
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
    /// # use awsx::parameter::Parameter;
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

    /// Check if a parameter is defined to use the previous value.
    pub fn is_previous_value(&self) -> bool {
        matches!(self, Parameter::PreviousValue { .. })
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

    /// Applies a default value to the current parameter, i.e. it turns a [`PreviousValue`] variant
    /// into a [`WithValue`] variant with the provided default value, but it keeps an existing
    /// [`WithValue`] unchanged. This method returns a new parameter.
    ///
    /// [`PreviousValue`]: #variant.PreviousValue
    /// [`WithValue`]: #variant.WithValue
    ///
    /// ## Examples
    ///
    /// ```
    /// # use awsx::parameter::Parameter;
    /// let with_value = Parameter::WithValue {
    ///     key: "MyParameter".to_owned(),
    ///     value: "Existing value".to_owned(),
    /// };
    /// let previous_value = Parameter::PreviousValue {
    ///     key: "MyParameter".to_owned(),
    /// };
    /// assert_eq!(
    ///     with_value,
    ///     with_value.with_default_value("new value".to_owned()),
    /// );
    /// assert_eq!(
    ///     Parameter::WithValue {
    ///         key: "MyParameter".to_owned(),
    ///         value: "new value".to_owned(),
    ///     },
    ///     previous_value.with_default_value("new value".to_owned()),
    /// );
    /// ```
    pub fn with_default_value(&self, default: String) -> Self {
        let mut new = self.clone();
        new.set_default_value(default);
        new
    }

    /// Applies a default value to the current parameter, i.e. it turns a [`PreviousValue`] variant
    /// into a [`WithValue`] variant with the provided default value, but it keeps an existing
    /// [`WithValue`] unchanged. This method changes the existing parameter in-place.
    ///
    /// [`PreviousValue`]: #variant.PreviousValue
    /// [`WithValue`]: #variant.WithValue
    pub fn set_default_value(&mut self, default: String) {
        use Parameter::*;
        match self {
            WithValue { .. } => {}
            PreviousValue { key, .. } => {
                *self = WithValue {
                    key: key.to_owned(),
                    value: default,
                }
            }
        }
    }

    /// Return a reference to the parameters key.
    ///
    /// This is a convenience function that abstracts matching over all variants, where `key` is a
    /// common field to all of them.
    pub fn key(&self) -> &str {
        use Parameter::*;
        match self {
            WithValue { key, .. } => key,
            PreviousValue { key, .. } => key,
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
                .ok_or_else(|| "Parameter needs to be provided in the form `Key=Value`".to_owned())?
                .to_owned(),
            value: split
                .next()
                .ok_or_else(|| "Parameter needs to be provided in the form `Key=Value`".to_owned())?
                .to_owned(),
        })
    }
}

/// This specialized serializer is used for the `Parameter::PreviousValue` variant internally.
/// Within the `PreviousValue` variant we do not track the `UsePreviousValue` variable since we
/// define it to be `true` when we instantiate this variant. During serialization we need to
/// reinject this field into the resulting JSON.
fn serialize_parameter_previousvalue<S>(key: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    #[derive(Serialize)]
    #[serde(untagged)]
    enum Parameter<'a> {
        PreviousValue {
            #[serde(rename = "ParameterKey")]
            key: &'a str,
            #[serde(rename = "UsePreviousValue")]
            use_previous_value: bool,
        },
    }

    Parameter::PreviousValue {
        key,
        use_previous_value: true,
    }
    .serialize(serializer)
}

/// A collection holding one or more stack or template parameters.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Parameters(pub(crate) IndexMap<String, Parameter>);

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

    /// Update all parameters in the current collection with the ones also available in the other
    /// collection.
    ///
    /// This can be used to override parameters, while avoiding to add new ones that the other
    /// collection might have defined.
    ///
    /// ```
    /// # use awsx::parameter::{Parameter, Parameters};
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
    /// # use awsx::parameter::{Parameter, Parameters};
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

    /// Set values for all parameters in the current collection based on the provided defaults,
    /// without modifying any pre-existing values or adding parameters that did not exist.
    ///
    /// ```
    /// # use awsx::parameter::{Parameter, Parameters};
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
    /// let defaults = Parameters::new(vec![
    ///     Parameter::WithValue {
    ///         key: "FirstParameter".to_owned(),
    ///         value: "Updated value".to_owned(),
    ///     },
    ///     Parameter::WithValue {
    ///         key: "SecondParameter".to_owned(),
    ///         value: "New value".to_owned(),
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
    /// parameters.apply_defaults(defaults);
    ///
    /// assert_eq!(
    ///     parameters,
    ///     vec![
    ///         Parameter::WithValue {
    ///             key: "FirstParameter".to_owned(),
    ///             value: "Initial value".to_owned(),
    ///         },
    ///         Parameter::WithValue {
    ///             key: "SecondParameter".to_owned(),
    ///             value: "New value".to_owned(),
    ///         },
    ///         Parameter::WithValue {
    ///             key: "ThirdParameter".to_owned(),
    ///             value: "New value".to_owned(),
    ///         },
    ///     ].into()
    /// );
    /// ```
    pub fn apply_defaults<P: IntoParameters>(&mut self, defaults: P) {
        for (key, parameter) in defaults.into_parameters().0 {
            if let Parameter::WithValue { value, .. } = parameter {
                self.0
                    .entry(key)
                    .and_modify(|parameter| parameter.set_default_value(value));
            }
        }
    }

    /// Set values for all parameters in the current collection based on the provided defaults,
    /// without modifying any pre-existing values or adding parameters that did not exist.
    ///
    /// In contrast to [`apply_defaults`], this does not mutate the existing collection, but rather
    /// returns a new copy.
    ///
    /// [`apply_defaults`]: #method.apply_defaults
    pub fn with_defaults<P: IntoParameters>(&self, defaults: P) -> Parameters {
        let mut this = self.clone();
        this.apply_defaults(defaults);
        this
    }

    /// Check if the other parameters are loosely equal to self.
    ///
    /// # Loose equality
    ///
    /// Loose equality in this case defines that when a parameter is present _with a value_ in the
    /// current set of parameters, the parameter is equal in the other set if it either has the same
    /// value, or if it is defined as the [`PreviousValue`] variant.
    ///
    /// [`PreviousValue`]: enum.Parameter.html#variant.PreviousValue
    pub fn loosely_equal(&self, other: &Parameters) -> bool {
        // A first check for the len allows us to quickly exit, should other be different.
        if self.len() != other.len() {
            return false;
        }

        // The next check is to see if the keys themselves are equal, i.e. self doesn't have keys
        // that other doesn't have and vice-versa.
        if !self.keys().sorted().eq(other.keys().sorted()) {
            return false;
        }

        // With the fact established that the keys are equal, we can now verify for every key in
        // self that the corresponding key in other is either strictly equal, or is references the
        // previous value.
        self.iter().all(|(key, parameter)| {
            let other_parameter = other
                .get(key)
                // Since we have established that the key must exist in other, we can safely unwrap.
                .unwrap();
            other_parameter.is_previous_value() || parameter == other_parameter
        })
    }

    /// Calculate the loose difference between this set of parameters and another set of parameters.
    ///
    /// # Returns
    ///
    /// Returns `None` if there are no differences, otherwise an instance of
    /// [`ParametersDifference`], from which you can identify:
    ///
    /// 1. parameters only available in this set of parameters (stored in [`left`])
    /// 2. parameters that exist in both sets but are unequal (stored in [`unequal`]) and
    /// 3. parameters that are only available in the other set of parameters (stored in [`right`]).
    ///
    /// # Loose equality
    ///
    /// Loose equality in this case defines that when a parameter is present _with a value_ in the
    /// current set of parameters, the parameter is equal in the other set if it either has the same
    /// value, or if it is defined as the [`PreviousValue`] variant.
    ///
    /// [`ParametersDifference`]: struct.ParametersDifference.html
    /// [`left`]: struct.ParameterDifference.html#structfield.left
    /// [`unequal`]: struct.ParameterDifference.html#structfield.unequal
    /// [`right`]: struct.ParameterDifference.html#structfield.right
    /// [`PreviousValue`]: enum.Parameter.html#variant.PreviousValue
    pub fn loose_difference<'a>(
        &'a self,
        other: &'a Parameters,
    ) -> Option<ParametersDifference<'a>> {
        let mut left: Vec<&'a Parameter> = Vec::new();
        let mut equal: Vec<&'a Parameter> = Vec::new();
        let mut unequal: Vec<(&'a Parameter, &'a Parameter)> = Vec::new();
        let mut right: Vec<&'a Parameter> = Vec::new();

        // Identify parameters available on `this` but not available on `other`.
        left.extend(self.iter().filter_map(|(key, parameter)| {
            if other.contains_key(key) {
                None
            } else {
                Some(parameter)
            }
        }));

        // Identify parameters which are available on both `this` and `other`, but are not loosely
        // equal.
        unequal.extend(self.iter().filter_map(|(key, parameter)| {
            other
                .get(key)
                .map(|other_parameter| {
                    (
                        other_parameter.is_previous_value() || parameter == other_parameter,
                        other_parameter,
                    )
                })
                .and_then(|(are_equal, other_parameter)| {
                    if are_equal {
                        // If the left parameter has a value, we push it into `equal`, otherwise we
                        // push `other_parameter`, which might have a value, or not. This ensures
                        // that when either parameter has a value, the parameter in `equal` will
                        // have the value.
                        if let Parameter::WithValue { .. } = &parameter {
                            equal.push(parameter);
                        } else {
                            equal.push(other_parameter);
                        }
                        None
                    } else {
                        Some((parameter, other_parameter))
                    }
                })
        }));

        // Identify parameters not available on `this` but available on `other`.
        right.extend(other.iter().filter_map(|(key, parameter)| {
            if self.contains_key(key) {
                None
            } else {
                Some(parameter)
            }
        }));

        // Should there be no differences, return `None`.
        if left.is_empty() && unequal.is_empty() && right.is_empty() {
            None
        } else {
            Some(ParametersDifference {
                left,
                equal,
                unequal,
                right,
            })
        }
    }
}

/// Struct to track the differences between two sets of [`Parameters`].
///
/// [`Parameters`]: struct.Parameters.html
#[derive(Debug)]
pub struct ParametersDifference<'a> {
    /// Parameters that are only available in the *left* parameter set
    pub left: Vec<&'a Parameter>,
    /// Parameters that are equal across the two parameter sets
    pub equal: Vec<&'a Parameter>,
    /// Parameters that are unequal across the two parameter sets
    pub unequal: Vec<(&'a Parameter, &'a Parameter)>,
    /// Parameters that are only available in the *right* parameter set
    pub right: Vec<&'a Parameter>,
}

impl From<Vec<Parameter>> for Parameters {
    fn from(parameters: Vec<Parameter>) -> Self {
        Parameters::new(parameters)
    }
}

impl From<Vec<&Parameter>> for Parameters {
    fn from(parameters: Vec<&Parameter>) -> Self {
        Parameters::new(
            parameters
                .into_iter()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
        )
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

impl ops::Deref for Parameters {
    type Target = IndexMap<String, Parameter>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Parameters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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

impl ops::Sub<&Parameters> for Parameters {
    type Output = Parameters;

    fn sub(mut self, rhs: &Parameters) -> Self::Output {
        rhs.0.keys().for_each(|key| {
            self.0.swap_remove(key);
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

impl IntoParameters for &Parameters {
    fn into_parameters(self) -> Parameters {
        self.to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parameter_previous_value() {
        let actual = Parameter::previous_value("MyKey".to_owned());
        let expected = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn parameter_is_previous_value() {
        let previous_value = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        let with_value = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };

        assert!(previous_value.is_previous_value());
        assert!(!with_value.is_previous_value());
    }

    #[test]
    fn parameter_from_cfnparameter() {
        let with_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            parameter_value: Some("my value".to_owned()),
            ..Default::default()
        };
        let with_value_expected = Some(Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        });
        assert_eq!(with_value_expected, Parameter::from(&with_value_actual));

        let previous_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            use_previous_value: Some(true),
            ..Default::default()
        };
        let previous_value_expected = Some(Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        });
        assert_eq!(
            previous_value_expected,
            Parameter::from(&previous_value_actual)
        );

        let resolved_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            resolved_value: Some("resolved value".to_owned()),
            ..Default::default()
        };
        let resolved_value_expected = None;
        assert_eq!(
            resolved_value_expected,
            Parameter::from(&resolved_value_actual)
        );

        let no_key_actual = rusoto_cloudformation::Parameter {
            parameter_key: None,
            ..Default::default()
        };
        let no_key_expected = None;
        assert_eq!(no_key_expected, Parameter::from(&no_key_actual));
    }

    #[test]
    fn parameter_from_as_previous_value_cfnparameter() {
        let with_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            parameter_value: Some("my value".to_owned()),
            ..Default::default()
        };
        let with_value_expected = Some(Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        });
        assert_eq!(
            with_value_expected,
            Parameter::from_as_previous_value(&with_value_actual)
        );

        let previous_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            use_previous_value: Some(true),
            ..Default::default()
        };
        let previous_value_expected = Some(Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        });
        assert_eq!(
            previous_value_expected,
            Parameter::from_as_previous_value(&previous_value_actual)
        );

        let resolved_value_actual = rusoto_cloudformation::Parameter {
            parameter_key: Some("MyKey".to_owned()),
            resolved_value: Some("resolved value".to_owned()),
            ..Default::default()
        };
        let resolved_value_expected = Some(Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        });
        assert_eq!(
            resolved_value_expected,
            Parameter::from_as_previous_value(&resolved_value_actual)
        );

        let no_key_actual = rusoto_cloudformation::Parameter {
            parameter_key: None,
            ..Default::default()
        };
        let no_key_expected = None;
        assert_eq!(
            no_key_expected,
            Parameter::from_as_previous_value(&no_key_actual)
        );
    }

    #[test]
    fn parameter_into_previous_value() {
        let with_value_actual = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        let with_value_expected = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        assert_eq!(with_value_expected, with_value_actual.into_previous_value());

        let previous_value_actual = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        let previous_value_expected = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        assert_eq!(
            previous_value_expected,
            previous_value_actual.into_previous_value()
        );
    }

    #[test]
    fn parameter_key() {
        let with_value_actual = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        let with_value_expected = "MyKey";
        assert_eq!(with_value_expected, with_value_actual.key());

        let previous_value_actual = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        let previous_value_expected = "MyKey";
        assert_eq!(previous_value_expected, previous_value_actual.key());
    }

    #[test]
    fn parameter_fromstr() {
        let with_space_actual = "MyKey=my value";
        let with_space_expected = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        assert_eq!(with_space_expected, with_space_actual.parse().unwrap());

        let with_equals_actual = "MyKey=value=value";
        let with_equals_expected = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "value=value".to_owned(),
        };
        assert_eq!(with_equals_expected, with_equals_actual.parse().unwrap());
    }

    #[test]
    fn parameter_serialize() {
        let with_value_actual = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        let with_value_expected = json!({
            "ParameterKey": "MyKey",
            "ParameterValue": "my value"
        });

        assert_eq!(
            with_value_expected,
            serde_json::to_value(with_value_actual).unwrap()
        );

        let previous_value_actual = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        let previous_value_expected = json!({
            "ParameterKey": "MyKey",
            "UsePreviousValue": true,
        });

        assert_eq!(
            previous_value_expected,
            serde_json::to_value(previous_value_actual).unwrap()
        );
    }

    #[test]
    fn parameter_deserialize() {
        let with_value_actual = json!({
            "ParameterKey": "MyKey",
            "ParameterValue": "my value"
        });
        let with_value_expected = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };

        assert_eq!(
            with_value_expected,
            serde_json::from_value(with_value_actual).unwrap()
        );

        let previous_value_actual = json!({
            "ParameterKey": "MyKey",
            "UsePreviousValue": true,
        });
        let previous_value_expected = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };

        assert_eq!(
            previous_value_expected,
            serde_json::from_value(previous_value_actual).unwrap()
        );

        let resolved_value_actual = json!({
            "ParameterKey": "MyKey",
            "ResolvedValue": "ResolvedValue"
        });
        let resolved_value_expected = Parameter::PreviousValue {
            key: "MyKey".to_owned(),
        };
        assert_eq!(
            resolved_value_expected,
            serde_json::from_value(resolved_value_actual).unwrap()
        );

        let no_key = json!({
            "ParameterValue": "MissingKey"
        });
        assert!(serde_json::from_value::<Parameter>(no_key).is_err());
    }

    #[test]
    fn parameters_new_empty() {
        let empty = Parameters::new(vec![]);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn parameters_new_nonempty() {
        let with_value = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        let previous_value = Parameter::PreviousValue {
            key: "OtherKey".to_owned(),
        };
        let non_empty = Parameters::new(vec![with_value.clone(), previous_value.clone()]);
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.len(), 2);
        assert_eq!(&with_value, non_empty.get("MyKey").unwrap());
        assert_eq!(&previous_value, non_empty.get("OtherKey").unwrap());
    }

    #[test]
    fn parameters_new_duplicate() {
        let with_value = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "my value".to_owned(),
        };
        let duplicate_key = Parameter::WithValue {
            key: "MyKey".to_owned(),
            value: "different value".to_owned(),
        };
        let non_empty = Parameters::new(vec![with_value, duplicate_key.clone()]);
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.len(), 1);
        assert_eq!(&duplicate_key, non_empty.get("MyKey").unwrap());
    }

    #[test]
    fn parameters_update() {
        let parameter1 = Parameter::WithValue {
            key: "Parameter1".to_owned(),
            value: "Value1".to_owned(),
        };
        let parameter2 = Parameter::PreviousValue {
            key: "Parameter2".to_owned(),
        };
        let mut parameters = Parameters::new(vec![parameter1, parameter2]);

        let parameter1 = Parameter::PreviousValue {
            key: "Parameter1".to_owned(),
        };
        let parameter2 = Parameter::WithValue {
            key: "Parameter2".to_owned(),
            value: "Value2".to_owned(),
        };
        let parameter3 = Parameter::PreviousValue {
            key: "Parameter3".to_owned(),
        };
        let new_parameters = vec![parameter1.clone(), parameter2.clone(), parameter3];

        // Verify that `update` and `updated` returned the same result.
        let parameters_updated = parameters.clone().updated(new_parameters.clone());
        parameters.update(new_parameters);
        assert_eq!(parameters, parameters_updated);

        // Verify the contents of the updated result.
        assert!(!parameters.is_empty());
        assert_eq!(parameters.len(), 2);
        assert_eq!(&parameter1, parameters.get("Parameter1").unwrap());
        assert_eq!(&parameter2, parameters.get("Parameter2").unwrap());
        assert!(parameters.get("Parameter3").is_none());
    }

    #[test]
    fn parameters_merge() {
        let parameter1 = Parameter::WithValue {
            key: "Parameter1".to_owned(),
            value: "Value1".to_owned(),
        };
        let parameter2 = Parameter::PreviousValue {
            key: "Parameter2".to_owned(),
        };
        let mut parameters = Parameters::new(vec![parameter1, parameter2]);

        let parameter1 = Parameter::PreviousValue {
            key: "Parameter1".to_owned(),
        };
        let parameter2 = Parameter::WithValue {
            key: "Parameter2".to_owned(),
            value: "Value2".to_owned(),
        };
        let parameter3 = Parameter::PreviousValue {
            key: "Parameter3".to_owned(),
        };
        let new_parameters = vec![parameter1.clone(), parameter2.clone(), parameter3.clone()];

        // Verify that `update` and `updated` returned the same result.
        let parameters_merged = parameters.clone().merged(new_parameters.clone());
        parameters.merge(new_parameters);
        assert_eq!(parameters, parameters_merged);

        // Verify the contents of the updated result.
        assert!(!parameters.is_empty());
        assert_eq!(parameters.len(), 3);
        assert_eq!(&parameter1, parameters.get("Parameter1").unwrap());
        assert_eq!(&parameter2, parameters.get("Parameter2").unwrap());
        assert_eq!(&parameter3, parameters.get("Parameter3").unwrap());
    }

    #[test]
    fn parameters_sub() {
        let parameter1 = Parameter::PreviousValue {
            key: "Parameter1".to_owned(),
        };
        let parameter2 = Parameter::WithValue {
            key: "Parameter2".to_owned(),
            value: "Value2".to_owned(),
        };
        let parameter3 = Parameter::PreviousValue {
            key: "Parameter3".to_owned(),
        };
        let parameter4 = Parameter::WithValue {
            key: "Parameter4".to_owned(),
            value: "Value4".to_owned(),
        };

        let left_parameters: Parameters =
            vec![parameter1.clone(), parameter2.clone(), parameter3.clone()].into();
        let right_parameters: Parameters = vec![parameter2, parameter3, parameter4].into();

        let parameters = left_parameters - &right_parameters;

        assert!(!parameters.is_empty());
        assert_eq!(parameters.len(), 1);
        assert_eq!(&parameter1, parameters.get("Parameter1").unwrap());
        assert!(parameters.get("Parameter2").is_none());
        assert!(parameters.get("Parameter3").is_none());
        assert!(parameters.get("Parameter4").is_none());
    }

    #[test]
    fn parameters_serialize() {
        let parameter1 = Parameter::PreviousValue {
            key: "Parameter1".to_owned(),
        };
        let parameter2 = Parameter::WithValue {
            key: "Parameter2".to_owned(),
            value: "Value2".to_owned(),
        };
        let parameter3 = Parameter::PreviousValue {
            key: "Parameter3".to_owned(),
        };

        let expected = json!([
            {
                "ParameterKey": "Parameter1",
                "UsePreviousValue": true
            },
            {
                "ParameterKey": "Parameter2",
                "ParameterValue": "Value2"
            },
            {
                "ParameterKey": "Parameter3",
                "UsePreviousValue": true
            }
        ]);
        let actual = Parameters::new(vec![parameter1, parameter2, parameter3]);

        assert_eq!(expected, serde_json::to_value(actual).unwrap());
    }

    #[test]
    fn parameters_deserialize() {
        let expected = Parameters::new(vec![
            Parameter::PreviousValue {
                key: "Parameter1".to_owned(),
            },
            Parameter::WithValue {
                key: "Parameter2".to_owned(),
                value: "Value2".to_owned(),
            },
            Parameter::PreviousValue {
                key: "Parameter3".to_owned(),
            },
            Parameter::PreviousValue {
                key: "Parameter4".to_owned(),
            },
        ]);
        let actual = json!([
            {
                "ParameterKey": "Parameter1",
                "UsePreviousValue": true
            },
            {
                "ParameterKey": "Parameter2",
                "ParameterValue": "Value2"
            },
            {
                "ParameterKey": "Parameter3",
                "UsePreviousValue": true
            },
            {
                "ParameterKey": "Parameter4",
                "ResolvedValue": "ResolvedValue"
            }
        ]);

        assert_eq!(expected, serde_json::from_value(actual).unwrap());
    }
}
