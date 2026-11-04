use clap::Parser;
use colored::Colorize;
use rust_i18n::t;
use semifold_core::ChangesetId;
use semifold_engine::{
    AppError, ApplyReport, ExecutionMode, PostVersionCommand, PostVersionCommandEvent,
    PostVersionCommandOutcome, Project, ReleaseApplyError, ReleaseApplyPlan,
    ReleaseExecutionOptions, SemifoldService, SystemDependencies, VersionWorkflowOutput,
    WorkflowExecutionMode, publish_plan::StdioPolicy, release::ReleaseBranchRenderError,
    release_apply::ReleasePrepareError,
};

use crate::cli::{
    repository_context,
    terminal::{StepOutcome, Terminal},
    workflow_output::{GithubOutputWriter, VERSION_OUTPUT_KEY},
};

#[derive(Parser, Debug)]
pub(crate) struct Version {
    #[clap(long, help = t!("cli.version.flags.allow_dirty"))]
    allow_dirty: bool,
}

pub(crate) async fn prepare_and_apply_release(
    project: &Project,
    release: semifold_core::ReleasePlan,
    dry_run: bool,
    terminal: &Terminal,
) -> anyhow::Result<ApplyReport> {
    let service = SemifoldService::new(SystemDependencies);
    let prepare_progress = terminal.progress(t!("cli.version.preparing").into_owned());
    let plan_result = service
        .prepare_release(
            project,
            release,
            &ReleaseExecutionOptions {
                collect_remote_metadata: !dry_run,
                repository: repository_context(),
            },
        )
        .await;
    let plan = match plan_result {
        Ok(plan) => plan,
        Err(error) => {
            log::debug!("Release preparation error: {error:?}");
            prepare_progress.finish(
                StepOutcome::Failed,
                t!("cli.version.preparation_failed").into_owned(),
            );
            let detail = match error {
                AppError::ReleasePrepare(ReleasePrepareError::ReleaseBranch(
                    ReleaseBranchRenderError::MatchesBase { branch },
                )) => t!("cli.release_branch_matches_base", branch = branch).into_owned(),
                error => format!("{:#}", anyhow::Error::new(error)),
            };
            return Err(anyhow::anyhow!(t!(
                "cli.version.prepare_failed",
                error = detail
            )));
        }
    };
    prepare_progress.finish(
        StepOutcome::Success,
        t!(
            "cli.version.prepared",
            packages = plan.release.packages().len(),
            edits = plan.release.file_edits().len(),
            changelogs = plan.changelogs.len()
        )
        .into_owned(),
    );
    render_release_plan_activity(&plan, dry_run, terminal);
    let output = VersionWorkflowOutput::from_release(
        &plan.release_context,
        plan.release_branch.clone(),
        if dry_run {
            WorkflowExecutionMode::DryRun
        } else {
            WorkflowExecutionMode::Apply
        },
    );
    let consumed_changesets = plan.release.consumed_changesets().len();
    let apply_progress = terminal.progress(if dry_run {
        t!("cli.version.validating").into_owned()
    } else {
        t!("cli.version.applying").into_owned()
    });
    let apply_result = apply_progress.suspend(|| {
        let mut command_progress = None;
        let mut callback = |command: &PostVersionCommand, event| {
            render_post_version_event(terminal, &mut command_progress, command, event);
        };
        service.apply_release_with_callback(
            plan,
            if dry_run {
                ExecutionMode::DryRun
            } else {
                ExecutionMode::Apply
            },
            &mut callback,
        )
    });
    let report = match apply_result {
        Ok(report) => report,
        Err(error) => {
            apply_progress.finish(
                StepOutcome::Failed,
                t!("cli.version.application_failed").into_owned(),
            );
            return Err(render_apply_error(error));
        }
    };
    apply_progress.finish(
        StepOutcome::Success,
        if dry_run {
            t!("cli.version.validated", edits = output.packages.len()).into_owned()
        } else {
            t!(
                "cli.version.applied",
                files = report
                    .file_edits
                    .as_ref()
                    .map_or(0, |edits| edits.applied.len()),
                changesets = consumed_changesets
            )
            .into_owned()
        },
    );
    terminal.blank();
    terminal.section(&t!("cli.version.result"));
    terminal.line(format!(
        "  {} {} {}",
        Terminal::cell(t!("cli.version.column_package"), 24),
        Terminal::cell(t!("cli.version.column_previous"), 16),
        t!("cli.version.column_version")
    ));
    for (package, version) in &output.packages {
        terminal.line(format!(
            "  {} {} {}",
            Terminal::cell(package.as_str(), 24).cyan().bold(),
            Terminal::cell(version.current_version.to_string(), 16).yellow(),
            version.next_version.to_string().green()
        ));
    }
    terminal.blank();
    terminal.summary(
        StepOutcome::Success,
        &if dry_run {
            t!("cli.version.dry_run_complete").into_owned()
        } else {
            t!("cli.version.complete").into_owned()
        },
    );
    terminal.fact(&t!("cli.version.release_branch"), &output.release_branch);
    GithubOutputWriter::from_environment()
        .write(VERSION_OUTPUT_KEY, &output)
        .map_err(|error| {
            anyhow::anyhow!(t!("cli.version.workflow_output_failed", error = error))
        })?;
    Ok(report)
}

fn render_release_plan_activity(plan: &ReleaseApplyPlan, dry_run: bool, terminal: &Terminal) {
    for package in &plan.remote_metadata_failures {
        terminal.warning(&t!(
            "cli.version.changelog_metadata_degraded",
            package = package.as_str()
        ));
    }
    for (package, failure) in &plan.github_failures {
        terminal.warning(&t!(
            "cli.github.package_failed",
            package = package.as_str(),
            error = super::github::render(failure)
        ));
    }
    let mut runnable = Vec::new();
    for planned in &plan.post_version_commands {
        let command = format!(
            "{} {}",
            planned.command.executable,
            planned.command.args.join(" ")
        );
        if dry_run && !planned.command.run_in_dry_run {
            terminal.warning(&t!(
                "cli.version.skip_post_version",
                command = command.magenta(),
                package = planned.package.as_str().cyan()
            ));
        } else {
            runnable.push(planned);
        }
    }
    if !runnable.is_empty() {
        let mut packages = Vec::new();
        for planned in runnable {
            if !packages.contains(&planned.package.as_str()) {
                packages.push(planned.package.as_str());
            }
        }
        terminal.line(t!(
            "cli.version.post_version_batch",
            packages = packages
                .into_iter()
                .map(|package| package.cyan().to_string())
                .collect::<Vec<_>>()
                .join(&t!("cli.version.post_version_package_separator"))
        ));
    }
}

fn render_post_version_event(
    terminal: &Terminal,
    progress: &mut Option<crate::cli::terminal::ProgressTask>,
    command: &PostVersionCommand,
    event: PostVersionCommandEvent,
) {
    match event {
        PostVersionCommandEvent::Started => {
            let rendered_command = format_command(command);
            if !inherits_terminal(command) {
                *progress = Some(
                    terminal.progress(
                        t!(
                            "cli.version.run_post_version",
                            command = rendered_command.magenta(),
                            package = command.package.as_str().cyan()
                        )
                        .into_owned(),
                    ),
                );
            }
        }
        PostVersionCommandEvent::Finished(outcome) => {
            let rendered_command = format_command(command);
            let (step_outcome, message) = match outcome {
                PostVersionCommandOutcome::Success => (
                    StepOutcome::Success,
                    t!(
                        "cli.version.post_version_complete",
                        command = rendered_command.magenta(),
                        package = command.package.as_str().cyan()
                    )
                    .into_owned(),
                ),
                PostVersionCommandOutcome::Failed => (
                    StepOutcome::Failed,
                    t!(
                        "cli.version.post_version_failed",
                        command = rendered_command.magenta(),
                        package = command.package.as_str().cyan()
                    )
                    .into_owned(),
                ),
            };
            if let Some(progress) = progress.take() {
                progress.finish(step_outcome, message);
            } else {
                terminal.step(step_outcome, &message);
            }
        }
    }
}

fn format_command(planned: &PostVersionCommand) -> String {
    format!(
        "{} {}",
        planned.command.executable,
        planned.command.args.join(" ")
    )
    .trim_end()
    .to_string()
}

fn inherits_terminal(planned: &PostVersionCommand) -> bool {
    planned.command.stdout == StdioPolicy::Inherit || planned.command.stderr == StdioPolicy::Inherit
}

fn render_apply_error(error: AppError) -> anyhow::Error {
    let (report, failure) = match error {
        AppError::ReleaseApply(error) => match *error {
            ReleaseApplyError::PostVersion { report, failure } => (report, failure),
            error => return AppError::ReleaseApply(Box::new(error)).into(),
        },
        error => return error.into(),
    };
    let files = report.file_edits.as_ref().map_or_else(
        || "-".to_string(),
        |report| {
            report
                .applied
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    let changesets = report
        .unconsumed_changesets
        .iter()
        .map(ChangesetId::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::anyhow!(t!(
        "cli.version.post_version_recovery",
        package = failure.package.as_str(),
        command = format!(
            "{} {}",
            failure.command.executable,
            failure.command.args.join(" ")
        ),
        error = failure.source,
        files = files,
        changesets = changesets
    ))
}

pub(crate) async fn run(opts: &Version, project: &Project, dry_run: bool) -> anyhow::Result<()> {
    let terminal = Terminal::detect();
    terminal.heading(&t!("cli.version.heading"));
    if dry_run {
        terminal.dry_run(&t!("cli.common.dry_run_banner"));
    }
    let service = SemifoldService::new(SystemDependencies);
    service
        .ensure_clean_worktree(project, opts.allow_dirty)
        .map_err(render_worktree_error)?;
    let planning = terminal.progress(t!("cli.version.planning").into_owned());
    let release = match service.plan_release(project) {
        Ok(release) => release,
        Err(error) => {
            planning.finish(
                StepOutcome::Failed,
                t!("cli.version.planning_failed").into_owned(),
            );
            return Err(error.into());
        }
    };
    planning.finish(
        StepOutcome::Success,
        t!("cli.version.planned", packages = release.packages().len()).into_owned(),
    );
    if release.consumed_changesets().is_empty() {
        terminal.summary(StepOutcome::Skipped, &t!("cli.version.empty_changesets"));
        return Ok(());
    }
    prepare_and_apply_release(project, release, dry_run, &terminal).await?;
    Ok(())
}

pub(crate) fn render_worktree_error(error: AppError) -> anyhow::Error {
    match error {
        AppError::DirtyWorktree => anyhow::anyhow!(t!("cli.dirty_repo")),
        AppError::GitOpen(error) => {
            anyhow::anyhow!(t!(
                "cli.version.git_open_failed",
                error = semifold_engine::git::diagnostic(&error)
            ))
        }
        AppError::GitStatus(error) => {
            anyhow::anyhow!(t!(
                "cli.version.git_status_failed",
                error = semifold_engine::git::diagnostic(&error)
            ))
        }
        error => error.into(),
    }
}
