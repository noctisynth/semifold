#![cfg_attr(test, allow(clippy::unwrap_used))]

use clap::Parser;
use log::LevelFilter;
use rust_i18n::t;
use semifold_engine::{ProjectLoadError, ProjectLocation};
use semifold_resolver::error::ResolveError;
use std::{ffi::OsString, process::ExitCode};

pub mod cli;
pub mod logger;
pub mod utils;

use cli::{Cli, Commands};

rust_i18n::i18n!("locales", fallback = "en");

pub fn run_cli() -> ExitCode {
    ExitCode::from(run_cli_with_args(std::env::args_os()))
}

pub fn run_cli_with_args<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    if let Some(locale) = sys_locale::get_locale() {
        rust_i18n::set_locale(&locale);
    }

    match run_with_cli(Cli::parse_from(args)) {
        Ok(()) => 0,
        Err(error) => {
            cli::terminal::Terminal::detect().error(&cli::github::render_command_error(&error));
            1
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    run_with_cli(Cli::parse())
}

fn run_with_cli(cli: Cli) -> anyhow::Result<()> {
    if cli.debug {
        logger::setup_logger(LevelFilter::Debug)?;
    } else {
        logger::setup_logger(LevelFilter::Info)?;
    }

    log::debug!(
        "Parsed command: {:?}",
        cli.command.as_ref().map(command_name)
    );

    if let Some(Commands::Mcp(mcp)) = &cli.command {
        utils::run_async(cli::mcp::run_mcp(mcp, cli.dry_run))?;
        return Ok(());
    }

    let current_dir = std::env::current_dir()?;
    let changeset_dir = std::env::var_os("CHANGESET_PATH").map(std::path::PathBuf::from);
    let location =
        ProjectLocation::discover_with_changeset_dir(&current_dir, changeset_dir.as_deref())
            .map_err(|error| anyhow::anyhow!(project_load_error_message(&error)))?;
    if let Some(Commands::Init(init)) = &cli.command {
        cli::init::run(init, &location, cli.dry_run)?;
        return Ok(());
    }
    if let Some(Commands::Config(config)) = &cli.command
        && let Some(result) = cli::config::run_before_project_load(config, &location, cli.dry_run)
    {
        result?;
        return Ok(());
    }
    let project = location
        .load()
        .map_err(|error| anyhow::anyhow!(project_load_error_message(&error)))?;
    log::debug!(
        "Loaded project with {} configured package(s)",
        project.config.packages.len()
    );

    match &cli.command {
        Some(Commands::Commit(commit)) => cli::commit::run(commit, &project, cli.dry_run)?,
        Some(Commands::Config(config)) => cli::config::run(config, &project, cli.dry_run)?,
        Some(Commands::Version(version)) => {
            utils::run_async(cli::version::run(version, &project, cli.dry_run))?
        }
        Some(Commands::Publish(publish)) => {
            utils::run_async(cli::publish::run(publish, &project, cli.dry_run))?
        }
        Some(Commands::CI(ci)) => utils::run_async(cli::ci::run(ci, &project, cli.dry_run))?,
        Some(Commands::Status(status)) => {
            utils::run_async(cli::status::run(status, &project, cli.dry_run))?
        }
        Some(Commands::Init(_) | Commands::Mcp(_)) | None => {}
    }

    Ok(())
}

fn project_load_error_message(error: &ProjectLoadError) -> String {
    let fallback = t!("cli.project_load_failed", error = error).into_owned();
    project_load_error_message_with_fallback(error, fallback)
}

pub(crate) fn project_load_error_message_with_fallback(
    error: &ProjectLoadError,
    fallback: String,
) -> String {
    let message = match error {
        ProjectLoadError::RepositoryOpenFailed { path, source } => {
            let key = if source.is_permission_denied() {
                "cli.git.access_denied"
            } else if source.is_not_found() {
                "cli.git.not_repository"
            } else {
                "cli.git.open_failed"
            };
            t!(
                key,
                path = path,
                error = semifold_engine::git::diagnostic(source)
            )
            .into_owned()
        }
        ProjectLoadError::ConfigInvalid {
            source: ResolveError::ReleaseBranchMatchesBase { branch, .. },
            ..
        } => t!("cli.release_branch_matches_base", branch = branch).into_owned(),
        ProjectLoadError::ConfigInvalid { source, .. } => source.to_string(),
        _ => fallback,
    };
    append_config_migration_hint(message, error)
}

pub(crate) fn append_config_migration_hint(
    mut message: String,
    error: &ProjectLoadError,
) -> String {
    if error.config_migration_may_help() {
        message.push('\n');
        message.push_str(&t!("cli.config_migration_hint"));
    }
    message
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Commit(_) => "commit",
        Commands::Init(_) => "init",
        Commands::Config(_) => "config",
        Commands::Version(_) => "version",
        Commands::Publish(_) => "publish",
        Commands::CI(_) => "ci",
        Commands::Status(_) => "status",
        Commands::Mcp(_) => "mcp",
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, str::FromStr};

    use toml_edit::{DocumentMut, Item, Table};

    fn locale_keys(source: &str) -> BTreeSet<String> {
        let document = DocumentMut::from_str(source).expect("locale fixture is valid TOML");
        let mut keys = BTreeSet::new();
        collect_keys(document.as_table(), "", &mut keys);
        keys
    }

    fn collect_keys(table: &Table, prefix: &str, keys: &mut BTreeSet<String>) {
        for (name, item) in table {
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}.{name}")
            };
            match item {
                Item::Table(child) => collect_keys(child, &path, keys),
                Item::Value(_) | Item::ArrayOfTables(_) | Item::None => {
                    keys.insert(path);
                }
            }
        }
    }

    #[test]
    fn english_and_chinese_locales_have_identical_keys() {
        assert_eq!(
            locale_keys(include_str!("../locales/en.toml")),
            locale_keys(include_str!("../locales/zh.toml"))
        );
    }

    #[test]
    fn repository_ci_workflow_exposes_semifold_step_outputs() {
        let workflow = include_str!("../../../.github/workflows/semifold-ci.yaml");

        assert!(workflow.contains("id: semifold"));
        assert!(workflow.contains("version: ${{ steps.semifold.outputs['semifold-version'] }}"));
        assert!(workflow.contains("publish: ${{ steps.semifold.outputs['semifold-publish'] }}"));
    }

    #[test]
    fn debug_paths_do_not_dump_configuration_or_github_payloads() {
        let config_dump = ["Loaded ", "config:"].concat();
        let event_dump = ["GITHUB_EVENT_PATH", " data:"].concat();
        assert!(!include_str!("lib.rs").contains(&config_dump));
        assert!(!include_str!("cli/status.rs").contains(&event_dump));
    }
}
