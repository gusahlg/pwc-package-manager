//! `pwc` — package, instance, build and run PWC mods (docs/spec/cli.md). A thin front end: every
//! command calls the library crates.
//!
//! Output conventions: a command's result (tables, paths, hashes) goes to stdout; progress,
//! status and warnings go to stderr (`-q` silences them, `-v` adds detail). Errors print
//! `error: <what>` and indented `cause:` lines and exit with status 1; usage errors exit with 2.

mod build;
mod config;
mod context;
mod instance;
mod output;
mod package;
mod store;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pwc_manifest::BuildProfile;

use crate::context::Ctx;

/// Package, instance, build and run Project Watt Cubed mods.
#[derive(Parser, Debug)]
#[command(
    name = "pwc",
    version,
    about,
    propagate_version = true,
    max_term_width = 100
)]
struct Cli {
    /// Instance to operate on (default: the active instance).
    #[arg(long, global = true, value_name = "NAME")]
    instance: Option<String>,
    /// Print more detail.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Print only results and errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Write config.toml: the PWC source checkout and the repositories.
    Setup {
        /// The PWC game checkout (default: a `project_watt_cubed` checkout next to this tooling).
        #[arg(long, value_name = "DIR")]
        pwc: Option<PathBuf>,
        /// A repository directory (repeatable; replaces the configured list).
        #[arg(long = "repository", value_name = "DIR")]
        repositories: Vec<PathBuf>,
    },
    /// Print the effective configuration and directories.
    Config,
    /// Manage repositories.
    #[command(subcommand)]
    Repo(RepoCommand),
    /// List packages available from repositories and the store.
    Search {
        /// Only packages whose id, name, description or keywords contain this text.
        text: Option<String>,
    },
    /// Every known version of a package with its metadata and dependencies.
    Info {
        /// Package id.
        id: String,
    },
    /// Validate, build, verify and inspect packages.
    #[command(subcommand)]
    Package(PackageCommand),
    /// Manage instances.
    #[command(subcommand)]
    Instance(InstanceCommand),
    /// Manage the mods of the instance.
    #[command(subcommand)]
    Mod(ModCommand),
    /// Re-resolve conservatively and write pwc.lock.
    Lock,
    /// Print the instance's environment hash.
    Env,
    /// Build the instance; print the executable path.
    Build(BuildArgs),
    /// Build if needed, then run the game (or the golden harness).
    Run {
        /// Run the golden harness instead of the game.
        #[arg(long)]
        golden: bool,
        /// Build profile (default: the instance's `[build] profile`).
        #[arg(long, value_enum)]
        profile: Option<ProfileArg>,
        /// Arguments passed to the game (after `--`).
        #[arg(last = true, value_name = "ARGS")]
        args: Vec<OsString>,
    },
    /// Inspect and maintain the store.
    #[command(subcommand)]
    Store(StoreCommand),
    /// Manage the build cache.
    #[command(subcommand)]
    Cache(CacheCommand),
}

#[derive(Subcommand, Debug)]
enum RepoCommand {
    /// Add a repository directory.
    Add {
        /// Directory whose children are package source directories or .pwcmod files.
        dir: PathBuf,
        /// Name (default: the directory name).
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a repository by name.
    Remove {
        /// Repository name.
        name: String,
    },
    /// List repositories.
    List,
}

#[derive(Subcommand, Debug)]
enum PackageCommand {
    /// Validate a source directory: manifest, licence policy, contents.
    Check {
        /// Package source directory (default: the current directory).
        dir: Option<PathBuf>,
    },
    /// Write `<out-dir>/<id>-<version>.pwcmod` and print its hash.
    Build {
        /// Package source directory (default: the current directory).
        dir: Option<PathBuf>,
        /// Output directory (default: `<dir>/dist`).
        #[arg(short = 'o', long = "out-dir", value_name = "OUT_DIR")]
        out_dir: Option<PathBuf>,
    },
    /// Fully verify a .pwcmod.
    Verify {
        /// The .pwcmod file.
        file: PathBuf,
        /// Expected hash (`sha256:…`).
        #[arg(long)]
        hash: Option<String>,
    },
    /// Print the manifest summary, hash and file list of a .pwcmod.
    Inspect {
        /// The .pwcmod file.
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum InstanceCommand {
    /// Create a new empty instance.
    Create {
        /// Instance name (`[a-z0-9][a-z0-9-]{0,63}`).
        name: String,
        /// Requirement on the PWC version (default `*`).
        #[arg(long, value_name = "REQ")]
        pwc: Option<String>,
        /// Make it the active instance.
        #[arg(long = "use")]
        use_it: bool,
    },
    /// List instances, marking the active one.
    List,
    /// Make an instance the active one.
    Use {
        /// Instance name.
        name: String,
    },
    /// Show an instance's requirements and locked packages.
    Show {
        /// Instance name (default: --instance or the active instance).
        name: Option<String>,
    },
    /// Delete an instance (its store entries remain until `pwc store gc`).
    Remove {
        /// Instance name.
        name: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ModCommand {
    /// Add mods (id, id@req, source directory or .pwcmod) and re-lock.
    Add {
        /// What to add.
        #[arg(required = true, value_name = "SPEC")]
        specs: Vec<String>,
    },
    /// Remove mods and re-lock.
    Remove {
        /// Package ids.
        #[arg(required = true, value_name = "ID")]
        ids: Vec<String>,
    },
    /// Re-resolve preferring the newest compatible versions (of the given ids, or of everything).
    Update {
        /// Package ids (default: all).
        #[arg(value_name = "ID")]
        ids: Vec<String>,
    },
    /// List the locked packages, marking direct requirements.
    List,
}

#[derive(Args, Debug)]
struct BuildArgs {
    /// Build profile (default: the instance's `[build] profile`).
    #[arg(long, value_enum)]
    profile: Option<ProfileArg>,
    /// Rebuild even when a cached build exists.
    #[arg(long)]
    force: bool,
    /// Generate the build workspace and print its directory instead of compiling.
    #[arg(long)]
    print_dir: bool,
}

#[derive(Subcommand, Debug)]
enum StoreCommand {
    /// Print `id version hash` for every store entry.
    List,
    /// Re-hash every entry and report corruption.
    Verify,
    /// Delete entries no instance's pwc.lock references.
    Gc {
        /// Only list what would be deleted.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CacheCommand {
    /// Delete build outputs and the shared target directory.
    Clean,
}

/// `--profile` values.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArg {
    /// Optimised.
    Release,
    /// Debug.
    Dev,
}

impl From<ProfileArg> for BuildProfile {
    fn from(p: ProfileArg) -> Self {
        match p {
            ProfileArg::Release => BuildProfile::Release,
            ProfileArg::Dev => BuildProfile::Dev,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            report(&err);
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let ctx = Ctx::new(cli.instance, cli.verbose, cli.quiet)?;
    match cli.command {
        Command::Setup { pwc, repositories } => config::setup(ctx, pwc, repositories)?,
        Command::Config => config::show(&ctx)?,
        Command::Repo(RepoCommand::Add { dir, name }) => config::repo_add(ctx, &dir, name)?,
        Command::Repo(RepoCommand::Remove { name }) => config::repo_remove(ctx, &name)?,
        Command::Repo(RepoCommand::List) => config::repo_list(&ctx),
        Command::Search { text } => package::search(&ctx, text.as_deref())?,
        Command::Info { id } => package::info(&ctx, &id)?,
        Command::Package(PackageCommand::Check { dir }) => package::check(&ctx, dir)?,
        Command::Package(PackageCommand::Build { dir, out_dir }) => {
            package::build(&ctx, dir, out_dir)?
        }
        Command::Package(PackageCommand::Verify { file, hash }) => {
            package::verify(&ctx, &file, hash.as_deref())?
        }
        Command::Package(PackageCommand::Inspect { file }) => package::inspect(&file)?,
        Command::Instance(InstanceCommand::Create { name, pwc, use_it }) => {
            instance::create(ctx, &name, pwc.as_deref(), use_it)?;
        }
        Command::Instance(InstanceCommand::List) => instance::list(&ctx)?,
        Command::Instance(InstanceCommand::Use { name }) => instance::use_instance(ctx, &name)?,
        Command::Instance(InstanceCommand::Show { name }) => instance::show(&ctx, name.as_deref())?,
        Command::Instance(InstanceCommand::Remove { name, yes }) => {
            instance::remove(ctx, &name, yes)?
        }
        Command::Mod(ModCommand::Add { specs }) => instance::mod_add(&ctx, &specs)?,
        Command::Mod(ModCommand::Remove { ids }) => instance::mod_remove(&ctx, &ids)?,
        Command::Mod(ModCommand::Update { ids }) => instance::mod_update(&ctx, &ids)?,
        Command::Mod(ModCommand::List) => instance::mod_list(&ctx)?,
        Command::Lock => instance::lock(&ctx)?,
        Command::Env => instance::env(&ctx)?,
        Command::Build(args) => build::build(
            &ctx,
            args.profile.map(Into::into),
            args.force,
            args.print_dir,
        )?,
        Command::Run {
            golden,
            profile,
            args,
        } => return build::run(&ctx, golden, profile.map(Into::into), &args),
        Command::Store(StoreCommand::List) => store::list(&ctx)?,
        Command::Store(StoreCommand::Verify) => store::verify(&ctx)?,
        Command::Store(StoreCommand::Gc { dry_run }) => store::gc(&ctx, dry_run)?,
        Command::Cache(CacheCommand::Clean) => build::cache_clean(&ctx)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `error: <what>` then one `  cause: …` line per underlying error. A cause whose text the
/// previous line already contains (wrappers such as `mod.toml: <cause>`) is not repeated.
fn report(err: &anyhow::Error) {
    eprintln!("error: {err}");
    let mut previous = err.to_string();
    for cause in err.chain().skip(1) {
        let text = cause.to_string();
        if previous.contains(&text) {
            continue;
        }
        eprintln!("  cause: {text}");
        previous = text;
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn run_passes_arguments_after_double_dash() {
        let cli =
            <Cli as clap::Parser>::try_parse_from(["pwc", "run", "--golden", "--", "--seed", "7"])
                .unwrap();
        match cli.command {
            super::Command::Run { golden, args, .. } => {
                assert!(golden);
                assert_eq!(args, ["--seed", "7"]);
            }
            other => panic!("parsed {other:?}"),
        }
    }

    #[test]
    fn global_options_work_after_the_subcommand() {
        let cli = <Cli as clap::Parser>::try_parse_from([
            "pwc",
            "mod",
            "list",
            "--instance",
            "survival",
            "-q",
        ])
        .unwrap();
        assert_eq!(cli.instance.as_deref(), Some("survival"));
        assert!(cli.quiet);
    }
}
