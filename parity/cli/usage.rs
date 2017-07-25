// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

macro_rules! otry {
	($e: expr) => (
		match $e {
			Some(ref v) => v,
			None => {
				return None;
			}
		}
	)
}
macro_rules! usage {
	(
		{
			$(
				$field_a:ident : $typ_a:ty,
			)*
		}
		{
			$(
				$field:ident : $typ:ty = $default:expr, or $from_config:expr,
			)*
		}
		{
			$(
				$field_s:ident : $typ_s:ty, display $default_s:expr, or $from_config_s:expr,
			)*
		}
		{
			$(
				CMD $subcommand:ident : $typ_subcommand:ty
				{
					$(
						CMD $subsubcommand:ident : $typ_subsubcommand:ty
						{
							$(
								ARG $subsubcommand_arg:ident : $typ_subsubcommand_arg:ty, $clap_subsubcommand_arg:expr,
							)*
						}
					)*

					$(
						ARG $subcommand_arg:ident : $typ_subcommand_arg:ty, $clap_subcommand_arg:expr,
					)*
				}
			)*
		}
		{
			$(
				$field_u:ident : $typ_u:ty = $default_u:expr, or $from_config_u:expr, $usage_u:expr,
			)*
		}
		{
			$(
				$field_flag_u:ident : $typ_flag_u:ty = $default_flag_u:expr, or $from_config_flag_u:expr, $usage_flag_u:expr,
			)*
		}
	) => {
		use toml;
		use std::{fs, io, process};
		use std::io::{Read, Write};
		use util::version;
		use docopt::{Docopt, Error as DocoptError};
		use clap::{Arg, App, SubCommand, AppSettings, Error as ClapError};
		use helpers::replace_home;

		#[derive(Debug)]
		pub enum ArgsError {
			Clap(ClapError),
			Decode(toml::de::Error),
			Config(String, io::Error),
		}

		impl ArgsError {
			pub fn exit(self) -> ! {
				match self {
					ArgsError::Clap(e) => e.exit(), // TODO PRINT ?
					ArgsError::Decode(e) => {
						println_stderr!("You might have supplied invalid parameters in config file.");
						println_stderr!("{}", e);
						process::exit(2)
					},
					ArgsError::Config(path, e) => {
						println_stderr!("There was an error reading your config file at: {}", path);
						println_stderr!("{}", e);
						process::exit(2)
					},
				}
			}
		}

		impl From<ClapError> for ArgsError {
			fn from(e: ClapError) -> Self {
				ArgsError::Clap(e)
			}
		}

		impl From<toml::de::Error> for ArgsError {
			fn from(e: toml::de::Error) -> Self {
				ArgsError::Decode(e)
			}
		}

		#[derive(Debug, PartialEq)]
		pub struct Args {
			$(
				pub $field_a: $typ_a,
			)*

			$(
				pub $field: $typ,
			)*

			$(
				pub $field_s: $typ_s,
			)*

			$(
				pub $subcommand: bool, /* @TODO hardcoded / remove :bool from the call */

				$(
					pub $subsubcommand: bool,
					$(
						pub $subsubcommand_arg: $typ_subsubcommand_arg,
					)*
				)*

				$(
					pub $subcommand_arg: $typ_subcommand_arg,
				)*
			)*

			$(
				pub $field_u: $typ_u,
			)*

			$(
				pub $field_flag_u: $typ_flag_u,
			)*
		}

		impl Default for Args {
			fn default() -> Self {
				Args {
					$(
						$field_a: Default::default(),
					)*

					$(
						$field: $default.into(),
					)*

					$(
						$field_s: Default::default(),
					)*

					$(
						$subcommand: Default::default(),
						$(
							$subsubcommand: Default::default(),
							$(
								$subsubcommand_arg: Default::default(),
							)*
						)*

						$(
							$subcommand_arg: Default::default(),
						)*
					)*

					$(
						$field_u: Default::default(),
					)*

					$(
						$field_flag_u: Default::default(),
					)*
				}
			}
		}

		#[derive(Default, Debug, PartialEq, Clone, Deserialize)]
		struct RawArgs {
			$(
				$field_a: $typ_a,
			)*
			$(
				$field: Option<$typ>,
			)*
			$(
				$field_s: Option<$typ_s>,
			)*
			$(
				$subcommand: bool, // @TODO HARDCODED / REMOVE TYPE FROM MACRO CALL
				
				$(
					$subsubcommand: bool,
					$(
						$subsubcommand_arg: $typ_subsubcommand_arg,
					)*
				)*

				$(
					$subcommand_arg: $typ_subcommand_arg,
				)*
			)*
			$(
				$field_u: Option<$typ_u>,
			)*
			$(
				$field_flag_u: bool, // @TODO HARDCODED / REMOVE TYPE FROM MACRO CALL
			)*
		}

		impl Args {

			pub fn parse<S: AsRef<str>>(command: &[S]) -> Result<Self, ArgsError> {
				let raw_args = RawArgs::parse(command)?;

				// Skip loading config file if no_config flag is specified
				if raw_args.flag_no_config {
					return Ok(raw_args.into_args(Config::default()));
				}

				let config_file = raw_args.flag_config.clone().unwrap_or_else(|| raw_args.clone().into_args(Config::default()).flag_config);
				let config_file = replace_home(&::dir::default_data_path(), &config_file);
				let config = match (fs::File::open(&config_file), raw_args.flag_config.is_some()) {
					// Load config file
					(Ok(mut file), _) => {
						println_stderr!("Loading config file from {}", &config_file);
						let mut config = String::new();
						file.read_to_string(&mut config).map_err(|e| ArgsError::Config(config_file, e))?;
						Self::parse_config(&config)?
					},
					// Don't display error in case default config cannot be loaded.
					(Err(_), false) => Config::default(),
					// Config set from CLI (fail with error)
					(Err(e), true) => {
						return Err(ArgsError::Config(config_file, e));
					},
				};

				Ok(raw_args.into_args(config))
			}

			#[cfg(test)]
			pub fn parse_without_config<S: AsRef<str>>(command: &[S]) -> Result<Self, ArgsError> {
				Self::parse_with_config(command, Config::default())
			}

			#[cfg(test)]
			fn parse_with_config<S: AsRef<str>>(command: &[S], config: Config) -> Result<Self, ArgsError> {
				RawArgs::parse(command).map(|raw| raw.into_args(config)).map_err(ArgsError::Clap)
			}

			fn parse_config(config: &str) -> Result<Config, ArgsError> {
				Ok(toml::from_str(config)?)
			}

			pub fn print_version() -> String {
				format!(include_str!("./version.txt"), version())
			}
		}

		impl RawArgs {
			fn into_args(self, config: Config) -> Args {
				let mut args = Args::default();
				$(
					args.$field_a = self.$field_a;
				)*
				$(
					args.$field = self.$field.or_else(|| $from_config(&config)).unwrap_or_else(|| $default.into());
				)*
				$(
					args.$field_s = self.$field_s.or_else(|| $from_config_s(&config)).unwrap_or(None);
				)*
				$(
					args.$subcommand = self.$subcommand;

					$(
						args.$subsubcommand = self.$subsubcommand;
						$(
							args.$subsubcommand_arg = self.$subsubcommand_arg;
						)*
					)*

					$(
						args.$subcommand_arg = self.$subcommand_arg;
					)*
				)*
				$(
					args.$field_u = self.$field_u.or_else(|| $from_config_u(&config)).unwrap_or_else(|| $default_u.into());
				)*
				$(
					// args.$field_flag_u = self.$field_flag_u.or_else(|| $from_config_flag_u(&config)).unwrap_or_else(|| $default_flag_u.into());

					// Presence of CLI switch || config || default
					args.$field_flag_u = self.$field_flag_u || $from_config_flag_u(&config).unwrap_or_else(|| $default_flag_u.into());
				)*

				args
			}

			pub fn parse<S: AsRef<str>>(command: &[S]) -> Result<Self, ClapError> {

				let matches = App::new("Parity")
				    	.setting(AppSettings::VersionlessSubcommands)
						.arg(Arg::with_name("version")
							.short("v")
							.long("version")
							.help(&Args::print_version()))
						.about(include_str!("./usage_header.txt"))
						$(
							.subcommand(
								SubCommand::with_name(&(stringify!(String::from($subcommand))[4..])) // @TODO remove () after &
								$(
									.subcommand(
										SubCommand::with_name(&(stringify!(String::from($subsubcommand))[stringify!(String::from($subcommand)).len()+1..]))
										$(
											.arg($clap_subsubcommand_arg(Arg::with_name(&(stringify!(String::from($subsubcommand_arg))[stringify!(String::from($subsubcommand)).len()+1..]))))
										)*
									)
								)*
								$(
									// todo @mut ? needs to move
									.arg($clap_subcommand_arg(Arg::with_name(&(stringify!(String::from($subcommand_arg))[stringify!(String::from($subcommand)).len()+1..]))))
								)*
							)
						)*
						.args(&[
							$(
								Arg::from_usage($usage_u),
							)*
							$(
								Arg::from_usage($usage_flag_u),
							)*
						])
						.get_matches_safe()?;

				let mut raw_args : RawArgs = Default::default();
				$(
					raw_args.$field_u = value_t!(matches, stringify!(String::from($field_u))[4..], $typ_u).ok();
				)*
				$(
					// Subcommand
					if let Some(submatches) = matches.subcommand_matches(&(stringify!(String::from($subcommand))[4..])) {
						raw_args.$subcommand = true;

						$(
							// Sub-subcommand
							if let Some(subsubmatches) = submatches.subcommand_matches(&(stringify!(String::from($subsubcommand))[stringify!(String::from($subcommand)).len()+1..])) {
								raw_args.$subsubcommand = true;

								// Sub-subcommand arguments
								$(
									raw_args.$subsubcommand_arg = value_t!(subsubmatches, stringify!(String::from($subsubcommand_arg))[stringify!(String::from($subsubcommand)).len()+1..], $typ_subsubcommand_arg).ok();
								)*
							}
							else {
								raw_args.$subsubcommand = false;
							}
						)*

						// Subcommand arguments
						$(
							raw_args.$subcommand_arg = value_t!(submatches, stringify!(String::from($subcommand_arg))[stringify!(String::from($subcommand)).len()+1..], $typ_subcommand_arg).ok();
						)*
					}
					else {
						raw_args.$subcommand = false;
					}
				)*
				$(
					raw_args.$field_flag_u = matches.is_present(&(stringify!(String::from($field_flag_u))[5..]));
				)*

				Ok(raw_args)				
			}
		}
	};
}
