// std
use std::{
	env,
	fs::{self, File},
	io::{Read, Write},
	path::PathBuf,
	result::Result as StdResult,
};
// crates.io
use anyhow::Result;
use clap::{
	builder::{
		styling::{AnsiColor, Effects},
		Styles,
	},
	Args, Parser, Subcommand, ValueEnum,
};
use regex::bytes::{Captures, Regex};
use walkdir::{DirEntry, WalkDir};

static mut VERBOSE: bool = false;

fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt::init();

	let mut args = env::args();

	if let Some("all") = env::args().nth(1).as_deref() {
		args.next();
	}

	Cli::parse_from(args).run()?;

	Ok(())
}

#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("CARGO_PKG_VERSION"),
		"-",
		env!("VERGEN_GIT_SHA"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	about,
	rename_all = "kebab",
	styles = styles(),
)]
struct Cli {
	#[command(subcommand)]
	subcmd: Subcmd,
	#[arg(global = true, long, short)]
	verbose: bool,
}
impl Cli {
	fn run(self) -> Result<()> {
		let Self { subcmd, verbose } = self;

		unsafe {
			VERBOSE = verbose;
		}

		subcmd.run()?;

		Ok(())
	}
}

#[derive(Debug, Subcommand)]
enum Subcmd {
	SetToolchain(SetToolchainCmd),
	Clean(CleanCmd),
}
impl Subcmd {
	fn run(self) -> Result<()> {
		// cargo-all
		use Subcmd::*;

		match self {
			SetToolchain(c) => c.run(),
			Clean(c) => c.run(),
		}?;

		Ok(())
	}
}

#[derive(Debug, Args)]
struct SetToolchainCmd {
	/// Set toolchain channel.
	///
	/// e.g. "nightly-2023-01-01"
	#[arg(value_name = "CHANNEL")]
	channel: String,
}
impl SetToolchainCmd {
	fn run(self) -> Result<()> {
		let Self { channel } = self;
		let verbose = unsafe { VERBOSE };
		let regex = Regex::new(r#"( *channel *= *)".*""#).unwrap();
		let set_version = |p: PathBuf| {
			if verbose {
				tracing::info!("setting: {}", p.display());
			}

			match File::open(&p) {
				Ok(mut f) => {
					let mut v = Vec::new();

					if let Err(e) = f.read_to_end(&mut v) {
						tracing::error!("skipped due to, {e:?}");
					}

					let v = regex.replace(&v, |c: &Captures| {
						let mut r = c[1].to_owned();

						r.push(b'"');
						r.extend(channel.as_bytes());
						r.push(b'"');

						r
					});
					let p_tmp = p.with_extension("cargo-all");

					match File::create(&p_tmp) {
						Ok(mut f) => {
							if let Err(e) = f.write_all(&v) {
								tracing::error!("skipped due to, {e:?}");
							}
							if let Err(e) = fs::rename(&p_tmp, p) {
								tracing::error!("skipped due to, {e:?}");

								if let Err(e) = fs::remove_file(p_tmp) {
									tracing::error!("failed to remove tmp file due to, {e:?}");
								}
							}
						},

						Err(e) => tracing::error!("skipped due to, {e:?}"),
					}
				},
				Err(e) => tracing::error!("skipped due to, {e:?}"),
			}
		};

		walk_with(&["rust-toolchain.toml", "rust-toolchain"], set_version)?;

		Ok(())
	}
}

#[derive(Debug, Args)]
struct CleanCmd {
	/// Profile.
	#[arg(value_enum, value_name = "NAME", default_value = "debug")]
	profile: Profile,
}
impl CleanCmd {
	fn run(self) -> Result<()> {
		let Self { profile } = self;
		let verbose = unsafe { VERBOSE };
		let rm = |p: PathBuf| {
			if let Err(e) = fs::remove_dir_all(p) {
				if verbose {
					tracing::warn!("skipped due to, {e:?}");
				}
			}
		};
		let rm_all = |p: PathBuf| {
			let p = p.parent().expect("already checked in previous step; qed").join("target");

			if verbose {
				tracing::info!("removing: {}", p.display());
			}

			rm(p);
		};
		let rm_profile = |p: PathBuf| {
			let p = p
				.parent()
				.expect("already checked in previous step; qed")
				.join("target")
				.join(profile.as_str());

			if verbose {
				tracing::info!("removing: {}", p.display());
			}

			rm(p);
		};

		match profile {
			Profile::All => walk_with(&["Cargo.toml"], rm_all)?,
			_ => walk_with(&["Cargo.toml"], rm_profile)?,
		}

		Ok(())
	}
}

#[derive(Clone, Debug, ValueEnum)]
enum Profile {
	Debug,
	Release,
	All,
}
impl Profile {
	fn as_str(&self) -> &'static str {
		// cargo-all
		use Profile::*;

		match self {
			Debug => "debug",
			Release => "release",
			All => unreachable!(),
		}
	}
}

fn styles() -> Styles {
	Styles::styled()
		.header(AnsiColor::Red.on_default() | Effects::BOLD)
		.usage(AnsiColor::Red.on_default() | Effects::BOLD)
		.literal(AnsiColor::Blue.on_default() | Effects::BOLD)
		.placeholder(AnsiColor::Green.on_default())
}

fn walk_with<F>(targets: &[&str], f: F) -> Result<()>
where
	F: Fn(PathBuf),
{
	let to_filter = |e: &DirEntry| {
		let n = e.file_name().to_string_lossy();

		!(n.starts_with('.') || n == "target")
	};
	let to_match = |r: StdResult<DirEntry, _>| match r {
		Ok(e) => {
			let n = e.file_name().to_string_lossy();

			if targets.iter().any(|t| *t == n) {
				Some(e.path().to_path_buf())
			} else {
				None
			}
		},
		Err(e) => {
			tracing::error!("skipped due to, {e:?}");

			None
		},
	};

	for e in
		WalkDir::new(env::current_dir()?).into_iter().filter_entry(to_filter).filter_map(to_match)
	{
		f(e);
	}

	Ok(())
}
