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

use awsx::{
    error::Error,
    parameter::{Parameter, Parameters},
    s3::S3Uploader,
    stack::Stack,
    template::Template,
};
use itertools::Itertools;
use rusoto_cloudformation::CloudFormationClient;
use rusoto_core::HttpClient;
use serde_json::json;
use std::{convert::TryFrom, fs::File, io::BufReader};
use structopt::StructOpt;

use crate::{
    util::{
        apply_defaults, apply_excludes_includes, generate_deployment_metadata,
        verify_changes_compatible, DeploymentMetadata,
    },
    AwsxOutput, AwsxProvider, Opt as GlobalOpt,
};

#[derive(Debug, StructOpt)]
pub(crate) struct Opt {
    #[structopt(long = "stack-name", help = "Name of the stack to update")]
    stack_name: String,
    #[structopt(long = "change-set-name", help = "Name for the new change set")]
    change_set_name: String,
    #[structopt(
        long = "role-arn",
        help = "IAM Role that AWS CloudFormation assumes when executing the change set"
    )]
    role_arn: Option<String>,
    #[structopt(long = "template-path", help = "Path to the new template")]
    template_path: String,
    #[structopt(
        short = "p",
        long = "parameters",
        conflicts_with = "parameter_path",
        help = "New parameters required by template",
        long_help = "New parameters required by template. Specify as multiple `Key=Value` pairs, \
                     where each key has to correspond to a parameter newly added to the template, \
                     i.e. the parameter can not be already defined on the stack.\n(If you specify \
                     this parameter, you cannot specify --parameter-path, --exclude or --include.)"
    )]
    parameters: Vec<Parameter>,
    #[structopt(
        long = "parameter-path",
        help = "Path to a JSON parameter file",
        conflicts_with = "parameters",
        long_help = "Path to a JSON parameter file. This file should be structured the same as the \
                     AWS CLI expects. The file can only contain parameters newly added to the \
                     template, unless the existing parameters are defined as \
                     `UsePreviousValue=true`.\n(If you specify this parameter, you cannot specify \
                     --parameters.)"
    )]
    parameter_path: Option<String>,
    #[structopt(
        long = "parameter-defaults-path",
        help = "Path to a JSON parameter file with defaults",
        long_help = "Path to a JSON parameter file, from which values will be taken if not \
                     specified in the regular parameter file. This file should be structured the \
                     same as the AWS CLI expects. If the provided path does not exist, no error is \
                     thrown, instead it will be simply ignored."
    )]
    parameter_defaults_path: Option<String>,
    #[structopt(
        long = "exclude",
        conflicts_with = "parameters",
        requires = "parameter-path",
        help = "Exclude parameters",
        long_help = "Exclude parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. If \
                     a parameter key matches any of the exclude-patterns, the parameter will not \
                     be applied."
    )]
    excludes: Vec<String>,
    #[structopt(
        long = "include",
        conflicts_with = "parameters",
        requires = "parameter-path",
        help = "Include parameters",
        long_help = "Include parameters based on the patterns provided. All patterns will be \
                     compiled into a regex-set, which will be used to match each parameter key. \
                     Every parameter key that doesn't match any of the include-patterns will not \
                     be applied.\n(Excludes are applied before includes, and you cannot include a \
                     parameter that was previously excluded.)"
    )]
    includes: Vec<String>,
    #[structopt(
        long = "only-new-parameters",
        help = "Only use newly added parameters from any given parameters",
        long_help = "By default, specifying parameters (either directly or through a path) will \
                     include all the parameters provided/defined, even if they are already defined \
                     on the destination stack. When updating a deployed template/stack this is not \
                     desired, which is why changeset creation fails if not forced in such cases. \
                     This option provides a convenience that whatever parameters are specified, \
                     only those are used that are actually new."
    )]
    only_new_parameters: bool,
    #[structopt(
        long = "force-create",
        help = "Force change set creation",
        long_help = "Force change set creation, even if the parameters supplied do not cover the \
                     newly required parameters exactly, or if the stack you are trying to deploy \
                     is not a direct child of the already deployed stack. This means that if you \
                     force change set creation, the created change set might contain parameter \
                     changes in addition to the template changes, and it might overwrite changes \
                     that are not part of the template you are trying to deploy."
    )]
    force_create: bool,
}

pub(crate) async fn update_stack(
    opt: &Opt,
    global_opt: &GlobalOpt,
    provider: AwsxProvider,
) -> Result<AwsxOutput, Error> {
    // Load the template
    let template = Template::new(&opt.template_path)?;

    // Create AWS clients
    let cfn = CloudFormationClient::new_with(
        HttpClient::new()?,
        provider.clone(),
        global_opt.aws_region.clone().unwrap_or_default(),
    );
    let s3 = S3Uploader::new(global_opt.aws_region.clone().unwrap_or_default(), provider);
    let s3_upload: Option<(&S3Uploader, &str)> = global_opt
        .s3_bucket_name
        .as_ref()
        .map(|bucket_name| (&s3, bucket_name.as_ref()));

    let stack = Stack::new(&opt.stack_name);

    // Retrieve the parameters defined on the template, as well as the current parameters defined on
    // the stack.
    let mut template_parameters = template.get_parameters_as_previous_value();
    let stack_parameters = stack.get_parameters_as_previous_value(&cfn).await?;

    // Identify newly added parameters, which are parameters defined on the template, but not on the
    // stack. (Parameters that are defined on the stack but not on the template, so the other way
    // around, are simply ignored, since they do not need to be set and will simply be removed
    // once the change-set is deployed.)
    let new_parameters = template_parameters.clone() - &stack_parameters;

    // Get the user provided parameters.
    let mut provided_parameters: Parameters = if let Some(parameter_path) = &opt.parameter_path {
        let file = File::open(parameter_path)?;
        let reader = BufReader::new(file);
        let parameters: Parameters = {
            let parameters: Parameters = serde_json::from_reader(reader).unwrap();
            parameters
                .values()
                .filter(|parameter| !parameter.is_previous_value())
                .collect::<Vec<_>>()
                .into()
        };
        apply_excludes_includes(parameters, &opt.excludes, &opt.includes)?
    } else {
        (&opt.parameters).into()
    };

    // Apply defaults if provided
    provided_parameters = apply_defaults(provided_parameters, &opt.parameter_defaults_path)?;
    // Apply defaults from template parameters. This ensures that any defaults specified in the
    // template itself will be honored and passed onto CloudFormation, unless they were specified
    // explicitly before.
    provided_parameters.apply_defaults(template_parameters.clone());

    if opt.only_new_parameters {
        provided_parameters = provided_parameters
            .values()
            .filter(|parameter| new_parameters.contains_key(parameter.key()))
            .collect::<Vec<_>>()
            .into();
    }

    // We need to ensure that the user has provided exactly the parameters that have been added.
    if !new_parameters
        .keys()
        .sorted()
        .eq(provided_parameters.keys().sorted())
    {
        if opt.force_create {
            eprintln!(
                "WARNING: all newly required parameters ({}) might not have been supplied, or some \
                 old or non-existent parameters were specified. The change set will be created \
                 since it was explicitly requested!",
                new_parameters.keys().join(", ")
            );
        } else {
            return Err(Error::InvalidParameters(format!(
                "all newly required parameters have to be provided ({}), and no old or \
                 non-existent parameters can be specified",
                new_parameters.keys().join(", ")
            )));
        }
    }

    // Update the template parameters with the provided parameters.
    template_parameters.update(provided_parameters);

    // Unless otherwise requested, we will update the deployment-metadata parameter
    if !global_opt.dont_update_deployment_metadata {
        if template_parameters.contains_key(&global_opt.deployment_metadata_parameter) {
            let previous_metadata_parameter = stack
                .get_parameter(&cfn, &global_opt.deployment_metadata_parameter)
                .await?;
            let previous_metadata =
                previous_metadata_parameter
                    .clone()
                    .and_then(|previous_metadata_parameter| {
                        DeploymentMetadata::try_from(previous_metadata_parameter.clone()).ok()
                    });
            let metadata = generate_deployment_metadata(
                previous_metadata_parameter,
                Some(&opt.template_path),
            )?;

            if let Some(previous_metadata) = previous_metadata {
                // Verify that the changes are compatible
                let changes_compatible =
                    verify_changes_compatible(&previous_metadata, &metadata, &opt.template_path)?;
                if !changes_compatible {
                    if opt.force_create {
                        eprintln!(
                            "WARNING: the changes you are trying to deploy are not a direct \
                             descendant of the currently deployed changes. The created change-set \
                             might overwrite and thus destroy the previously deployed changes."
                        );
                    } else {
                        return Err(Error::InvalidTemplate(
                            "the template provided is not a direct descendant of the currently \
                             deployed template, creating a changeset might overwrite previously \
                             deployed changes"
                                .to_string(),
                        ));
                    }
                }
            }

            template_parameters.insert(
                global_opt.deployment_metadata_parameter.clone(),
                Parameter::WithValue {
                    key: global_opt.deployment_metadata_parameter.clone(),
                    value: metadata.to_string(),
                },
            );
        } else {
            eprintln!(
                "WARNING: an update to the deployment-metadata parameter '{}' was requested, but \
                 the template that should be deployed does not have this parameter. The change-set \
                 will be created, although without any metadata.",
                &global_opt.deployment_metadata_parameter,
            );
        }
    }

    // Create the change set for the new template, including the new parameters.
    template
        .create_change_set(
            &cfn,
            &opt.change_set_name,
            &opt.stack_name,
            &template_parameters,
            opt.role_arn.as_ref().map(|role_arn| &**role_arn),
            s3_upload,
            false,
        )
        .await?;

    Ok(AwsxOutput {
        human_readable: format!(
            "Change set {} creation started successfully",
            opt.change_set_name
        ),
        structured: json!({
            "success": true,
            "change_set_name": opt.change_set_name,
        }),
        successful: true,
    })
}
