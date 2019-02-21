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

use awsx::{error::Error, parameter::Parameters};
use regex::RegexSet;

pub(crate) fn apply_excludes_includes(
    mut parameters: Parameters,
    excludes: &Vec<String>,
    includes: &Vec<String>,
) -> Result<Parameters, Error> {
    if !excludes.is_empty() {
        let excludes = RegexSet::new(excludes)?;
        parameters = parameters
            .values()
            .filter(|parameter| !excludes.is_match(parameter.key()))
            .collect::<Vec<_>>()
            .into()
    }
    if !includes.is_empty() {
        let includes = RegexSet::new(includes)?;
        parameters = parameters
            .values()
            .filter(|parameter| includes.is_match(parameter.key()))
            .collect::<Vec<_>>()
            .into()
    }

    Ok(parameters)
}
