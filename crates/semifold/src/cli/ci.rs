use semifold_changelog::github::{GitHubFailure, GitHubOperation};
use std::env;

use anyhow::Context as _;
use clap::Parser;
use octocrab::{Octocrab, params};
use rust_i18n::t;

use crate::cli::{
    publish,
    terminal::{StepOutcome, Terminal},
    version,
};
use semifold_core::ReleaseContext;
use semifold_engine::{
    Project, SemifoldService, SystemDependencies,
    release::{
        ReleaseBranchRenderError, ReleasePullRequestContext, render_configured_release_branch,
        render_release_commit_message, render_release_pull_request,
        render_release_pull_request_title,
    },
};

#[derive(Debug, Parser)]
pub struct CI;

pub(crate) async fn run(_ci: &CI, project: &Project, dry_run: bool) -> anyhow::Result<()> {
    let terminal = Terminal::detect();
    terminal.heading(&t!("cli.ci.heading"));
    if dry_run {
        terminal.dry_run(&t!("cli.common.dry_run_banner"));
    }
    let config = &project.config;
    if std::env::var("GITHUB_ACTIONS").is_err() {
        return Err(anyhow::anyhow!(t!("cli.ci.not_ci_environment")));
    }

    let ref_name = env::var("GITHUB_REF_NAME").context("GITHUB_REF_NAME is not set")?;

    log::debug!("GITHUB_REF_NAME: {}", ref_name);

    let is_base_branch = ref_name == config.branches.base;
    if !is_base_branch {
        terminal.summary(StepOutcome::Skipped, &t!("cli.ci.not_base_branch"));
        return Ok(());
    }

    let release_plan = SemifoldService::new(SystemDependencies).plan_release(project)?;
    if release_plan.consumed_changesets().is_empty() {
        terminal.line(t!("cli.ci.no_changesets_publish"));
        publish::publish(project, dry_run, true, &terminal).await?;
        return Ok(());
    }

    let release_context = ReleaseContext::from_plan(&release_plan);
    let release_branch = render_configured_release_branch(&config.branches, &release_context)
        .map_err(|error| match error {
            ReleaseBranchRenderError::MatchesBase { branch } => {
                anyhow::anyhow!(t!("cli.release_branch_matches_base", branch = branch))
            }
            error => anyhow::anyhow!(t!("cli.ci.release_branch_invalid", error = error)),
        })?;
    let release_commit_message =
        render_release_commit_message(config.release.commit_message.as_deref(), &release_context)
            .map_err(|error| anyhow::anyhow!(t!("cli.ci.commit_message_invalid", error = error)))?;
    let release_pull_request_title = render_release_pull_request_title(
        config.release.pull_request_title.as_deref(),
        &release_context,
    )
    .map_err(|error| anyhow::anyhow!(t!("cli.ci.pull_request_title_invalid", error = error)))?;

    let push = if dry_run {
        None
    } else {
        Some(super::ci_push::PushAdapter::open(
            project.root.as_std_path(),
        )?)
    };

    let semifold_engine::ApplyReport {
        changelogs: changelogs_map,
        file_edits: _,
        unconsumed_changesets: _,
    } = version::prepare_and_apply_release(project, release_plan, dry_run, &terminal).await?;
    let oversized_body_notice = t!("cli.ci.oversized_body_notice");
    let pull_request_context = ReleasePullRequestContext {
        release: &release_context,
        branch: release_branch,
        changelogs: changelogs_map,
        oversized_body_notice: &oversized_body_notice,
    };
    let pull_request =
        render_release_pull_request(release_pull_request_title, &pull_request_context);

    if dry_run {
        terminal.summary(StepOutcome::Success, &t!("cli.ci.dry_run_complete"));
        return Ok(());
    }

    let github_repo = env::var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY is not set")?;
    let (owner, repo_name) = github_repo
        .split_once('/')
        .ok_or(anyhow::anyhow!(t!("cli.ci.github_repo_invalid_format")))?;
    let github_token = env::var("GITHUB_TOKEN").context("GITHUB_TOKEN is not set")?;
    let octocrab = Octocrab::builder()
        .personal_token(&*github_token)
        .build()
        .map_err(|error| GitHubFailure::new(GitHubOperation::Initialize, error))?;

    let repository =
        semifold_engine::git::open_repository(project.root.as_std_path()).map_err(|error| {
            anyhow::anyhow!(t!(
                "cli.ci.local_git_failed",
                error = semifold_engine::git::diagnostic(&error)
            ))
        })?;
    semifold_engine::git::commit_release(
        &repository,
        &pull_request_context.branch,
        &release_commit_message,
    )
    .map_err(|error| anyhow::anyhow!(t!("cli.ci.local_git_failed", error = error)))?;
    if let Some(push) = push {
        push.force_push(&github_token, &pull_request_context.branch)
            .map_err(|error| {
                anyhow::anyhow!(t!(
                    "cli.github.push_failed",
                    error = super::github::render_command_error(&error)
                ))
            })?;
    }

    let base_branch = &config.branches.base;
    let head = format!("{}:{}", owner, pull_request_context.branch);
    let pulls = octocrab.pulls(owner, repo_name);
    let existing_prs = pulls
        .list()
        .state(params::State::Open)
        .head(head)
        .base(base_branch)
        .send()
        .await
        .map_err(|error| GitHubFailure::new(GitHubOperation::ListPullRequests, error))?
        .take_items();

    if existing_prs.is_empty() {
        terminal.line(t!("cli.ci.no_existing_pr"));
        pulls
            .create(
                &pull_request.title,
                &pull_request_context.branch,
                base_branch,
            )
            .body(&pull_request.body)
            .send()
            .await
            .map_err(|error| GitHubFailure::new(GitHubOperation::CreatePullRequest, error))?;
    } else {
        let pr = existing_prs
            .first()
            .ok_or_else(|| anyhow::anyhow!(t!("cli.ci.existing_pr_missing")))?;
        terminal.line(t!("cli.ci.existing_pr_found", number = pr.number));
        pulls
            .update(pr.number)
            .title(&pull_request.title)
            .body(&pull_request.body)
            .send()
            .await
            .map_err(|error| GitHubFailure::new(GitHubOperation::UpdatePullRequest, error))?;
    }

    terminal.summary(StepOutcome::Success, &t!("cli.ci.complete"));
    Ok(())
}
