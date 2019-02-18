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

//! This module enables type-safe handling of CloudFormation templates.

use md5::{Digest, Md5};
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};

use crate::parameter::*;

/// Represents a CloudFormation template, based on some source file.
///
/// It holds the loaded file contents as well as the parsed template parameters.
pub struct Template {
    filename: String,
    contents: Vec<u8>,
    parameters: Parameters,
}

impl Template {
    /// Loads a template from a file.
    ///
    /// **Note:** this will load the template into memory.
    pub fn new<S: AsRef<str>>(filename: S) -> Template {
        let contents = load_file(filename.as_ref());
        let parameters = serde_yaml::from_slice::<CloudFormationTemplate>(&contents)
            .unwrap()
            .parameters
            .keys()
            .map(String::to_owned)
            .map(Parameter::previous_value)
            .collect::<Vec<_>>()
            .into();

        Template {
            filename: filename.as_ref().to_owned(),
            contents,
            parameters,
        }
    }

    /// Return the path to the file loaded.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Return the contents of the loaded template.
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }

    /// Generate and return a MD5 checksum of the file contents.
    pub fn checksum_md5hex(&self) -> String {
        let mut contents_md5 = Md5::new();
        let mut bufreader = BufReader::new(self.contents.as_slice());
        io::copy(&mut bufreader, &mut contents_md5).unwrap();
        format!("{:x}", contents_md5.result())
    }

    /// Create the change set input for the loaded template and a given list of parameters.
    ///
    /// This function will validate that the parameter list matches what the template expects, and
    /// will return an error if this isn't the case.
    pub fn create_change_set_input(
        &self,
        stack_name: &str,
        parameters: Parameters,
    ) -> Result<rusoto_cloudformation::CreateChangeSetInput, ()> {
        if self.validate_parameters(&parameters) {
            Ok(rusoto_cloudformation::CreateChangeSetInput {
                stack_name: stack_name.to_owned(),
                // TODO create S3 uploader
                template_url: Some("url".to_owned()),
                capabilities: Some(vec![
                    "CAPABILITY_IAM".to_owned(),
                    "CAPABILITY_AUTO_EXPAND".to_owned(),
                ]),
                parameters: Some(parameters.into()),
                ..Default::default()
            })
        } else {
            Err(())
        }
    }

    /// Get the parameters expected by the template.
    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }

    fn validate_parameters(&self, parameters: &Parameters) -> bool {
        let mut keys = self.parameters.keys().collect::<Vec<_>>();
        keys.sort();
        let mut other_keys = parameters.keys().collect::<Vec<_>>();
        other_keys.sort();

        keys == other_keys
    }
}

impl std::fmt::Debug for Template {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Template")
            .field("filename", &self.filename)
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CloudFormationTemplate {
    parameters: HashMap<String, TemplateParameter>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TemplateParameter {
    #[serde(rename = "Type")]
    _type: String,
    default: Option<String>,
}

fn load_file(filename: &str) -> Vec<u8> {
    let file = File::open(filename).unwrap();
    let metadata = file.metadata().unwrap();
    let mut reader = BufReader::new(file);
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    reader.read_to_end(&mut contents).unwrap();
    contents
}
