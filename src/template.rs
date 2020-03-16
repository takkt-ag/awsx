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
use rusoto_cloudformation::{CloudFormation, CreateChangeSetInput, CreateChangeSetOutput};
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};

use crate::{error::Error, parameter::*, s3::S3Uploader};

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
    pub fn new<S: AsRef<str>>(filename: S) -> Result<Template, Error> {
        let contents = load_file(filename.as_ref())?;
        let parameters = serde_yaml::from_slice::<CloudFormationTemplate>(&contents)
            .map_err(|error| Error::TemplateDeserializationFailed(error.into()))?
            .parameters
            .into_iter()
            .map(|(name, parameter)| {
                if let Some(default) = parameter.default {
                    Parameter::WithValue {
                        key: name.clone(),
                        value: default,
                    }
                } else {
                    Parameter::PreviousValue { key: name.clone() }
                }
            })
            .collect::<Vec<_>>()
            .into();

        Ok(Template {
            filename: filename.as_ref().to_owned(),
            contents,
            parameters,
        })
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
    pub fn checksum_md5hex(&self) -> Result<String, Error> {
        let mut contents_md5 = Md5::new();
        let mut bufreader = BufReader::new(self.contents.as_slice());
        io::copy(&mut bufreader, &mut contents_md5)?;
        Ok(format!("{:x}", contents_md5.result()))
    }

    /// Create the change set input for the loaded template and a given list of parameters.
    ///
    /// This function will validate that the parameter list matches what the template expects, and
    /// will return an error if this isn't the case. If the stack doesn't exist but should be
    /// created, set `create_stack` to `true`.
    pub async fn create_change_set(
        &self,
        cfn: &dyn CloudFormation,
        name: &str,
        stack_name: &str,
        parameters: &Parameters,
        role_arn: Option<&str>,
        s3_upload: Option<(&S3Uploader, &str)>,
        create_stack: bool,
    ) -> Result<CreateChangeSetOutput, Error> {
        if self.validate_parameters(&parameters) {
            let mut create_change_set_input = CreateChangeSetInput {
                stack_name: stack_name.to_owned(),
                change_set_name: name.to_owned(),
                capabilities: Some(vec![
                    "CAPABILITY_IAM".to_owned(),
                    "CAPABILITY_NAMED_IAM".to_owned(),
                    "CAPABILITY_AUTO_EXPAND".to_owned(),
                ]),
                change_set_type: if create_stack {
                    Some("CREATE".to_owned())
                } else {
                    Some("UPDATE".to_owned())
                },
                role_arn: role_arn.map(ToOwned::to_owned),
                parameters: Some(parameters.into()),
                ..Default::default()
            };

            // Upload the template if the S3 configuration was provided, use the template as-is
            // otherwise.
            if let Some((s3_uploader, bucket_name)) = s3_upload {
                let url = self.upload_to_s3(s3_uploader, bucket_name).await?;
                create_change_set_input.template_url = Some(url);
            } else {
                create_change_set_input.template_body = Some(
                    String::from_utf8(self.contents.clone())
                        .expect("Template is not well formatted UTF8"),
                );
            }

            cfn.create_change_set(create_change_set_input)
                .await
                .map_err(Into::into)
        } else {
            Err(Error::InvalidParameters(
                "the template expected other parameters than were provided".to_owned(),
            ))
        }
    }

    /// Get the parameters expected by the template.
    pub fn get_parameters(&self) -> &Parameters {
        &self.parameters
    }

    /// Get the parameters of type `Parameter::PreviousValue`.
    ///
    /// **Note:** this will recreate the parameter collection.
    pub fn get_parameters_as_previous_value(&self) -> Parameters {
        self.parameters
            .iter()
            .map(|(_, parameter)| parameter.clone().into_previous_value())
            .collect::<Vec<_>>()
            .into()
    }

    /// Upload the current template to S3.
    ///
    /// # Deduplication
    ///
    /// This will deduplicate the uploaded template by hashing the template contents using MD5, and
    /// using the result as the filename. This means that if a template that duplicates one already
    /// on S3 is to be uploaded, the filenames will match and the file will be deduplicated
    /// automatically.
    ///
    /// This behaviour is identical to the AWS CLI, which means the deduplication works across both
    /// tools.
    pub async fn upload_to_s3(&self, s3: &S3Uploader, bucket_name: &str) -> Result<String, Error> {
        let key = format!("{}.template", self.checksum_md5hex()?);
        let url = s3
            .upload(bucket_name, &key, self.contents.clone().into())
            .await?;
        Ok(url)
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

fn load_file(filename: &str) -> Result<Vec<u8>, Error> {
    let file = File::open(filename)?;
    let metadata = file.metadata()?;
    let mut reader = BufReader::new(file);
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    reader.read_to_end(&mut contents)?;
    Ok(contents)
}
