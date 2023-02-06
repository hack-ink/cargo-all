// std
use std::{env, fs, path::PathBuf, result::Result as StdResult};
// crates.io
use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("VERGEN_BUILD_SEMVER"),
		"-",
		env!("VERGEN_GIT_SHA_SHORT"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	about,
	rename_all = "kebab",
)]
struct Cli {
	#[command(subcommand)]
	subcmd: Subcmd,
}
impl Cli {
	fn run(self) -> Result<()> {
		tracing_subscriber::fmt::init();

		self.subcmd.run()?;

		Ok(())
	}
}

#[derive(Debug, Subcommand)]
enum Subcmd {
	Toolchain(ToolchainCmd),
	Clean(CleanCmd),
}
impl Subcmd {
	fn run(self) -> Result<()> {
		// cargo-all
		use Subcmd::*;

		match self {
			Toolchain(c) => c.run(),
			Clean(c) => c.run(),
		}?;

		Ok(())
	}
}

#[derive(Debug, Args)]
struct ToolchainCmd {}
impl ToolchainCmd {
	fn run(self) -> Result<()> {
		Ok(())
	}
}

#[derive(Debug, Args)]
struct CleanCmd {
	/// Profile.
	#[arg(value_enum, long, short, value_name = "NAME", default_value = "debug")]
	profile: Profile,
}
impl CleanCmd {
	fn run(self) -> Result<()> {
		fn walk_with<F>(f: F) -> Result<()>
		where
			F: Fn(PathBuf),
		{
			let to_filter = |e: &DirEntry| {
				let n = e.file_name().to_string_lossy();

				!(n.starts_with('.') || n == "target")
			};
			let to_match = |r: StdResult<DirEntry, _>| match r {
				Ok(e) =>
					if e.file_name().to_string_lossy().ends_with("Cargo.toml") {
						e.path().parent().map(|p| p.to_path_buf())
					} else {
						None
					},
				Err(e) => {
					tracing::error!("skipped due to, {e:?}");

					None
				},
			};

			for e in WalkDir::new(env::current_dir()?)
				.into_iter()
				.filter_entry(to_filter)
				.filter_map(to_match)
			{
				f(e);
			}

			Ok(())
		}

		let Self { profile } = self;
		let rm = |p: PathBuf| {
			if let Err(e) = fs::remove_dir_all(p) {
				tracing::warn!("skipped due to, {e:?}");
			}
		};
		let rm_all = |p: PathBuf| {
			let p = p.join("target");

			tracing::info!("removing: {}", p.display());

			rm(p);
		};
		let rm_profile = |p: PathBuf| {
			let p = p.join("target").join(profile.as_str());

			tracing::info!("removing: {}", p.display());

			rm(p);
		};

		match profile {
			Profile::All => walk_with(rm_all)?,
			_ => walk_with(rm_profile)?,
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

fn main() -> Result<()> {
	let cli = Cli::parse();

	cli.run()?;

	Ok(())
}
