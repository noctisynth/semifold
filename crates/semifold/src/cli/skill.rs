use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, ValueEnum};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

const PROTOCOL_VERSION: u32 = 1;
const TASK_INDEX: &str = include_str!("../../assets/skill/tasks.json");
const ENTRYPOINT: &str = include_str!("../../assets/skill/SKILL.md");

#[derive(Parser, Debug)]
pub(crate) struct Skill {
    #[arg(global = true, long, value_enum, help = t!("cli.skill.flags.language"))]
    language: Option<Language>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Language {
    En,
    Zh,
}

impl Language {
    fn from_locale(locale: &str) -> Self {
        if locale
            .split(['-', '_', '.'])
            .next()
            .is_some_and(|language| language.eq_ignore_ascii_case("zh"))
        {
            Self::Zh
        } else {
            Self::En
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(about = t!("cli.skill.commands.list"))]
    List,
    #[command(about = t!("cli.skill.commands.get"))]
    Get {
        #[arg(help = t!("cli.skill.flags.task"))]
        task: String,
    },
    #[command(about = t!("cli.skill.commands.export"))]
    Export {
        #[arg(long, value_name = "DIR", help = t!("cli.skill.flags.output"))]
        output: PathBuf,
    },
}

#[derive(Deserialize)]
struct Task {
    task: String,
    commands: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Response<T> {
    protocol_version: u32,
    cli_version: &'static str,
    language: &'static str,
    data: T,
}

#[derive(Serialize)]
struct Summary {
    task: String,
    summary: String,
}

#[derive(Serialize)]
struct Guide {
    task: String,
    commands: Vec<String>,
    instructions: String,
}

#[derive(Serialize)]
struct Export {
    output: PathBuf,
    files: [&'static str; 1],
    written: bool,
}

pub(crate) fn run(skill: &Skill, dry_run: bool) -> anyhow::Result<()> {
    let language = skill.language.unwrap_or_else(|| {
        let locale = sys_locale::get_locale();
        Language::from_locale(locale.as_deref().unwrap_or("en"))
    });
    let locale = language.as_str();
    match &skill.command {
        Command::List => {
            let summaries: Vec<_> = tasks(locale)?
                .into_iter()
                .map(|task| {
                    let key = format!("cli.skill.guides.{}_summary", task.task);
                    Summary {
                        task: task.task,
                        summary: t!(&key, locale = locale).into_owned(),
                    }
                })
                .collect();
            respond(summaries, locale)
        }
        Command::Get { task } => {
            let selected = tasks(locale)?
                .into_iter()
                .find(|candidate| candidate.task == *task)
                .ok_or_else(|| {
                    anyhow::anyhow!(t!(
                        "cli.skill.errors.unknown_task",
                        locale = locale,
                        task = task
                    ))
                })?;
            let key = format!("cli.skill.guides.{}", selected.task);
            let shared = t!("cli.skill.guides.shared", locale = locale);
            let body = t!(&key, locale = locale);
            respond(
                Guide {
                    task: selected.task,
                    commands: selected.commands,
                    instructions: format!("{}\n\n{}", shared.trim_end(), body),
                },
                locale,
            )
        }
        Command::Export { output } => respond(export(output, dry_run, locale)?, locale),
    }
}

fn tasks(locale: &str) -> anyhow::Result<Vec<Task>> {
    serde_json::from_str(TASK_INDEX).map_err(|error| {
        anyhow::anyhow!(t!(
            "cli.skill.errors.invalid_bundle",
            locale = locale,
            error = error
        ))
    })
}

fn respond(data: impl Serialize, locale: &'static str) -> anyhow::Result<()> {
    let response = Response {
        protocol_version: PROTOCOL_VERSION,
        cli_version: env!("CARGO_PKG_VERSION"),
        language: locale,
        data,
    };
    let mut bytes = serde_json::to_vec(&response).map_err(|error| {
        anyhow::anyhow!(t!(
            "cli.skill.errors.output_failed",
            locale = locale,
            error = error
        ))
    })?;
    bytes.push(b'\n');
    io::stdout().lock().write_all(&bytes).map_err(|error| {
        anyhow::anyhow!(t!(
            "cli.skill.errors.output_failed",
            locale = locale,
            error = error
        ))
    })
}

fn export(path: &Path, dry_run: bool, locale: &str) -> anyhow::Result<Export> {
    let absolute = std::path::absolute(path).map_err(|error| export_error(path, locale, error))?;
    if absolute.to_str().is_none() {
        anyhow::bail!(t!("cli.skill.errors.invalid_path", locale = locale));
    }
    require_missing(&absolute, locale)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!(t!("cli.skill.errors.invalid_path", locale = locale)))?;
    let parent = parent
        .canonicalize()
        .map_err(|error| export_error(&absolute, locale, error))?;
    if !parent.is_dir() {
        anyhow::bail!(t!("cli.skill.errors.invalid_path", locale = locale));
    }
    let name = absolute
        .file_name()
        .ok_or_else(|| anyhow::anyhow!(t!("cli.skill.errors.invalid_path", locale = locale)))?;
    let output = parent.join(name);
    if output.to_str().is_none() {
        anyhow::bail!(t!("cli.skill.errors.invalid_path", locale = locale));
    }
    require_missing(&output, locale)?;

    if !dry_run {
        // create_dir and create_new both refuse existing targets, including links.
        fs::create_dir(&output).map_err(|error| export_error(&output, locale, error))?;
        let entry = output.join("SKILL.md");
        // Preserve partial output on failure: another process may have changed the directory.
        let mut file =
            File::create_new(&entry).map_err(|error| export_error(&output, locale, error))?;
        file.write_all(ENTRYPOINT.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| export_error(&output, locale, error))?;
    }

    Ok(Export {
        output,
        files: ["SKILL.md"],
        written: !dry_run,
    })
}

fn require_missing(path: &Path, locale: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!(t!(
            "cli.skill.errors.output_exists",
            locale = locale,
            path = path.display()
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(export_error(path, locale, error)),
    }
}

fn export_error(path: &Path, locale: &str, error: io::Error) -> anyhow::Error {
    anyhow::anyhow!(t!(
        "cli.skill.errors.export_failed",
        locale = locale,
        path = path.display(),
        error = error
    ))
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn content_language_normalizes_system_locales_without_global_mutation() {
        for locale in ["zh", "zh-CN", "zh_TW.UTF-8", "ZH-Hant-HK"] {
            assert_eq!(Language::from_locale(locale).as_str(), "zh");
        }
        for locale in ["", "C", "en-US", "ja-JP", "zhx"] {
            assert_eq!(Language::from_locale(locale).as_str(), "en");
        }
    }
}
