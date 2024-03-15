use crate::{
    error::job_error::{
        NotAValidYamlKeyForStringValue, NotYqV4, Result, U8VectorToString, YqAppendToArrayCommand,
        YqAppendToObjectCommand, YqCommandExec, YqDeleteObjectCommand, YqMergeCommand,
        YqSetCommand, YqVersionCommand,
    },
    regex::Regex,
    vec_to_strings,
};
use snafu::{ensure, ResultExt};
use std::{
    fmt::Display,
    ops::Deref,
    path::Path,
    process::{Command, Output},
    str,
};

/// This is a container for the String of an input yaml key.
#[derive(Clone)]
pub(crate) struct YamlKey(String);

impl TryFrom<&str> for YamlKey {
    type Error = crate::error::job_error::Error;

    /// This generates a YamlKey after vetting it. A yaml dot notation
    /// pattern is considered a valid input.
    fn try_from(value: &str) -> Result<Self> {
        let value_as_string = value.to_string();
        // A string where '.' followed by any character, any number of times,
        // again the set may be repeated any number of times. E.g: ".a.x.p.j".
        let yaml_key_regex = r"^(\..+)+$";
        ensure!(
            Regex::new(yaml_key_regex)?.is_match(value),
            NotAValidYamlKeyForStringValue {
                key: value_as_string
            }
        );
        Ok(YamlKey(value_as_string))
    }
}

impl Deref for YamlKey {
    type Target = String;

    /// This Deref implementation lets the inner String stand in for the YamlKey.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// This type is for running `yq` v4.x.y commands.
pub(crate) struct YqV4 {
    /// This is the name of the binary, for use when running `yq` Commands.
    command_name: String,
}

impl YqV4 {
    /// Run the `yq -V` command to check if yq exists and it's version is v4.x.y.
    pub(crate) fn new() -> Result<Self> {
        let yq_v4 = Self {
            command_name: String::from("yq"),
        };

        let yq_version_arg = "-V".to_string();

        let yq_version_output = yq_v4.command_output(vec![yq_version_arg.clone()])?;

        ensure!(
            yq_version_output.status.success(),
            YqVersionCommand {
                command: yq_v4.command_as_str().to_string(),
                arg: yq_version_arg,
                std_err: str::from_utf8(yq_version_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        // Yq v4.x.y, else die.
        let yq_version_regex = r"^(.+4\.[0-9]+\.[0-9]+.*)$".to_string();
        ensure!(
            Regex::new(yq_version_regex.as_str())?.is_match(
                str::from_utf8(yq_version_output.stdout.as_slice())
                    .context(U8VectorToString)?
                    .trim()
            ),
            NotYqV4
        );

        Ok(yq_v4)
    }

    /// Append objects to yaml arrays.
    pub(crate) fn append_to_array<V, P>(&self, key: YamlKey, value: V, filepath: P) -> Result<()>
    where
        V: Display + Sized,
        P: AsRef<Path>,
    {
        let yq_append_to_array_args = vec_to_strings![
            "-i",
            format!(r#"{} += [{value}]"#, key.as_str()),
            filepath.as_ref().to_string_lossy()
        ];
        let yq_append_to_array_output = self.command_output(yq_append_to_array_args.clone())?;

        ensure!(
            yq_append_to_array_output.status.success(),
            YqAppendToArrayCommand {
                command: self.command_as_str().to_string(),
                args: yq_append_to_array_args,
                std_err: str::from_utf8(yq_append_to_array_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(())
    }

    /// Append fields to yaml objects.
    pub(crate) fn append_to_object<V, P>(&self, key: YamlKey, value: V, filepath: P) -> Result<()>
    where
        V: Display + Sized,
        P: AsRef<Path>,
    {
        let yq_append_to_object_args = vec_to_strings![
            "-i",
            format!(r#"{} += {value}"#, key.as_str()),
            filepath.as_ref().to_string_lossy()
        ];
        let yq_append_to_object_output = self.command_output(yq_append_to_object_args.clone())?;

        ensure!(
            yq_append_to_object_output.status.success(),
            YqAppendToObjectCommand {
                command: self.command_as_str().to_string(),
                args: yq_append_to_object_args,
                std_err: str::from_utf8(yq_append_to_object_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(())
    }

    /// Use the yq 'del' operator to delete objects from a yaml file.
    pub(crate) fn delete_object<P>(&self, key: YamlKey, filepath: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let yq_delete_object_args = vec_to_strings![
            "-i",
            format!(r#"del({})"#, key.as_str()),
            filepath.as_ref().to_string_lossy()
        ];
        let yq_delete_object_output = self.command_output(yq_delete_object_args.clone())?;

        ensure!(
            yq_delete_object_output.status.success(),
            YqDeleteObjectCommand {
                command: self.command_as_str().to_string(),
                args: yq_delete_object_args,
                std_err: str::from_utf8(yq_delete_object_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(())
    }

    // TODO:
    // 1. Arrays are treated like unique values on their own, and high_priority is preferred over
    //    low_priority. Arrays are not merged, if the object in the array member is identical to an
    //    existing member in the other file, we cannot decide on the key-value-pairs to compare to
    //    check for identical array objects.
    // 2. If the default value in the upgrade target has changed, and the user has also changed the
    //    default value (of the upgrade source), which one should be preferred?
    /// Run yq evaluate on two file together. The latter (in the yq command args) file's values are
    /// preferred over those of the other file's. In case there are values absent in the latter one
    /// which exist in the other file, the values of the other file are taken. The 'latter' file in
    /// this function is the one called 'high_priority' and the other file is the 'low_priority'
    /// one.
    /// E.g:
    ///       high_priority file:
    ///       ===================
    ///       foo:
    ///         bar: "foobar"
    ///         baz:
    ///           - "alpha"
    ///           - "beta"
    ///
    ///       low_priority file:
    ///       ==================
    ///       foo:
    ///         bar: "foobaz"
    ///         baz:
    ///           - "gamma"
    ///           - "delta"
    ///       friend: "ferris"
    ///
    ///       result:
    ///       =======
    ///       foo:
    ///         bar: "foobar"
    ///         baz:
    ///           - "alpha"
    ///           - "beta"
    ///         friend: "ferris"
    ///
    /// Special case: When the default value has changed, and the user has not customised that
    /// option, special upgrade values yaml updates have to be added to single out specific cases
    /// and migrate the older default to the newer one. E.g.: the .io_engine.logLevel is set to
    /// 'info' deliberately if the upgrade source file is seen to contain the value
    /// 'info,io_engine=info' and the target yaml is seen to not contain it.
    pub(crate) fn merge_files<P, Q>(&self, high_priority: P, low_priority: Q) -> Result<Vec<u8>>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let yq_merge_args: Vec<String> = vec_to_strings![
            "ea",
            r#". as $item ireduce ({}; . * $item )"#,
            low_priority.as_ref().to_string_lossy(),
            high_priority.as_ref().to_string_lossy()
        ];
        let yq_merge_output = self.command_output(yq_merge_args.clone())?;

        ensure!(
            yq_merge_output.status.success(),
            YqMergeCommand {
                command: self.command_as_str().to_string(),
                args: yq_merge_args,
                std_err: str::from_utf8(yq_merge_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(yq_merge_output.stdout)
    }

    /// This sets in-place yaml values in yaml files.
    pub(crate) fn set_literal_value<V, P>(&self, key: YamlKey, value: V, filepath: P) -> Result<()>
    where
        V: Display + Sized,
        P: AsRef<Path>,
    {
        // Command for use during yq file update
        let mut command = self.command();

        // Assigning value based on if it needs quotes around it or not.
        // Strings require quotes.
        let value = match format!("{value}").as_str() {
            // Boolean yaml values do not need quotes.
            "true" => "true".to_string(),
            "false" => "false".to_string(),
            // Null doesn't need quotes as well.
            "null" => "null".to_string(),
            // What remains is integers and strings
            something_else => 'other: {
                // If it's an integer, then no quotes.
                if something_else.parse::<i64>().is_ok() {
                    break 'other something_else.to_string();
                }

                // Preserve special characters for a string.
                // Ref: https://mikefarah.gitbook.io/yq/usage/tips-and-tricks#special-characters-in-strings
                command.env("VAL", something_else);
                "strenv(VAL)".to_string()
            }
        };

        let yq_set_args = vec_to_strings![
            "-i",
            format!(r#"{} = {value}"#, key.as_str()),
            filepath.as_ref().to_string_lossy()
        ];
        let yq_set_output = command
            .args(yq_set_args.clone())
            .output()
            .context(YqCommandExec {
                command: self.command_as_str().to_string(),
                args: yq_set_args.clone(),
            })?;

        ensure!(
            yq_set_output.status.success(),
            YqSetCommand {
                command: self.command_as_str().to_string(),
                args: yq_set_args,
                std_err: str::from_utf8(yq_set_output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(())
    }

    /// Returns an std::process::Command using the command_as_str member's value.
    fn command(&self) -> Command {
        Command::new(self.command_name.clone())
    }

    /// This executes a command and returns the output.
    fn command_output(&self, args: Vec<String>) -> Result<Output> {
        self.command()
            .args(args.clone())
            .output()
            .context(YqCommandExec {
                command: self.command_as_str().to_string(),
                args,
            })
    }

    /// The binary name of the `yq` command.
    fn command_as_str(&self) -> &str {
        self.command_name.as_str()
    }
}
