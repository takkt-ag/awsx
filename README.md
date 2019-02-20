# awsx

awsx is a command-line utility providing helpful commands that the default AWS CLI does not provide.
It does not replace the AWS CLI, rather it is meant to work in conjunction with it.

## Usage

### `awsx`

```
awsx 0.1.0
KAISER+KRAFT EUROPA GmbH

USAGE:
    awsx [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --assume-role-arn <assume_role_arn>                Optional role to assume before executing AWS API calls
        --aws-access-key-id <aws_access_key_id>            AWS Access Key ID used for AWS API authentication
        --aws-region <aws_region>                          Region the AWS API calls should be performed in
        --aws-secret-access-key <aws_secret_access_key>    AWS Secret Access Key used for AWS API authentication
        --output-format <output_format>
            Specify the format of the application output [possible values: human, human-readable, structured, json, yml,
            yaml]
        --s3-bucket-name <s3_bucket_name>                  Name of the S3 bucket used for storing templates

SUBCOMMANDS:
    help                        Prints this message or the help of the given subcommand(s)
    identify-new-parameters     Show new template parameters not present on the stack
    override-parameters         Update specified parameters on an existing stack
    update-deployed-template    Update an existing stack with a new template
```

### `awsx identify-new-parameters`

```
awsx-identify-new-parameters 0.1.0
KAISER+KRAFT EUROPA GmbH
Show all new parameters defined on the template, but not present on the stack. This subcommand does not create a change
set, and performs only read-only actions.

USAGE:
    awsx identify-new-parameters --stack-name <stack_name> --template-path <template_path>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --stack-name <stack_name>          Name of the stack to update
        --template-path <template_path>    Path to the new template
```

### `awsx override-parameters`

```
awsx-override-parameters 0.1.0
KAISER+KRAFT EUROPA GmbH
Update specified parameters on an existing stack, without updating the underlying template. Only the specified
parameters will be updated, with all other parameters staying unchanged. NOTE: this will only create a change set that
will not be automatically executed.

USAGE:
    awsx override-parameters [OPTIONS] --change-set-name <change_set_name> --stack-name <stack_name>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --change-set-name <change_set_name>               Name for the new change set
    -p, --parameter-overrides <parameter_overrides>...    Parameters to override
        --stack-name <stack_name>                         Name of the stack to update
```

### `awsx update-deployed-template`

```
awsx-update-deployed-template 0.1.0
KAISER+KRAFT EUROPA GmbH
Update an existing stack with a new template, without updating any parameters already defined on the stack. You can and
have to supply parameters that are newly added. NOTE: this will only create a change set that will not be automatically
executed.

USAGE:
    awsx update-deployed-template [OPTIONS] --change-set-name <change_set_name> --stack-name <stack_name> --template-path <template_path>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --change-set-name <change_set_name>    Name for the new change set
    -p, --parameters <parameters>...           New parameters required by template
        --stack-name <stack_name>              Name of the stack to update
        --template-path <template_path>        Path to the new template
```

## License

awsx is licensed under the Apache License, Version 2.0, (see [LICENSE](LICENSE) or <https://www.apache.org/licenses/LICENSE-2.0>).

awsx internally makes use of various open-source projects.
You can find a full list of these projects and their licenses in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in awsx by you, as defined in the Apache-2.0 license, shall be licensed under the Apache License, Version 2.0, without any additional terms or conditions.

We require code submitted to be formatted with Rust's default rustfmt formatter (CI will automatically verified if your code is formatted correctly).
We are using unstable rustfmt formatting rules, which requires running the formatter with a nightly toolchain, which you can do as follows:

```sh
$ rustup toolchain install nightly
$ cargo +nightly fmt
```

(Building and running awsx itself can and should happen with the stable toolchain.)

Additionally we are also checking whether there are any clippy warnings in your code.
You can run clippy locally with:

```sh
$ cargo clippy --workspace --lib --bins --tests --all-targets -- -Dwarnings
```

There can be occasions where newer versions of clippy warn about code you haven't touched.
In such cases we'll try to get those warnings resolved before merging your changes, or work together with you to get them resolved in your merge request.

## Affiliation

This project has no official affiliation with Amazon Web Services, Inc., Amazon.com, Inc., or any of its affiliates.
"Amazon Web Services" is a trademark of Amazon.com, Inc. or its affiliates in the United States and/or other countries.
