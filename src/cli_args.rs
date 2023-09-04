pub use crate::config::Config;
use clap::{error::ErrorKind, Parser};

#[derive(Debug, Parser)]
pub struct CliArgs {
    /// The input argument is a single line in CSV format as follows:
    ///
    /// 1.1.1.1,3,1000
    ///
    /// Where the first column is the IPv4 address, the second column is the number of requests to send,
    /// and the third column is the interval in milliseconds between sent requests.
    #[arg(value_parser = ConfigParser)]
    pub config: Config,
}

/// Custom chapter ID parser, which would be used for the default value for the `chapter_id` field.
#[derive(Clone)]
pub struct ConfigParser;

impl clap::builder::TypedValueParser for ConfigParser {
    type Value = Config;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let Some(arg_value) = value.to_str() else {
            let err = clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            eprintln!("Value error: {}", err);
            return Err(err);
        };
        let c = arg_value.parse().map_err(|e| {
            eprintln!("ConfigError error: {}", e);
            clap::Error::new(ErrorKind::InvalidValue).with_cmd(cmd)
        })?;
        Ok(c)
    }
}
