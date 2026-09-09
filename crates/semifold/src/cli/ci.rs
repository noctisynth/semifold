use semifold_changelog::github::{GitHubFailure, GitHubOperation};
use std::env;

use anyhow::Context as _;
use clap::Parser;
use git2::{Cred, IndexAddOption, PushOptions, RemoteCallbacks, Repository};
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

fn build_callbacks(token: &str) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    let token = token.to_string();

    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        if username_from_url.is_some() {
            Cred::userpass_plaintext(&token, "")
        } else {
            Cred::userpass_plaintext("x-access-token", &token)
        }
    });

    callbacks
}

fn force_push_release(repo: &Repository, token: &str, branch: &str) -> anyhow::Result<()> {
    let callbacks = build_callbacks(token);
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = repo.find_remote("origin").context("find remote origin")?;
    let ref_spec = format!("+refs/heads/{branch}:refs/heads/{branch}", branch = branch);
    remote.push(&[&ref_spec], Some(&mut push_opts))?;
    Ok(())
}

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

    let repo = Repository::open(project.root.as_std_path())
        .map_err(|_| anyhow::anyhow!(t!("cli.ci.git_repo_not_initialized")))?;
    let mut git_config = repo.config()?;
    git_config.set_str("user.name", "github-actions[bot]")?;
    git_config.set_str("user.email", "github-actions[bot]@users.noreply.github.com")?;

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

    let head = repo.head()?;
    let commit = head.peel_to_commit()?;

    let base_branch = &config.branches.base;
    repo.branch(&pull_request_context.branch, &commit, true)?;
    repo.set_head(&format!("refs/heads/{}", pull_request_context.branch))?;
    repo.checkout_head(None)?;

    let mut index = repo.index()?;
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = repo.signature()?;
    let parent_commit = repo.head()?.peel_to_commit()?;
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &release_commit_message,
        &tree,
        &[&parent_commit],
    )?;

    force_push_release(&repo, &github_token, &pull_request_context.branch).map_err(|error| {
        anyhow::anyhow!(t!(
            "cli.github.push_failed",
            error = semifold_changelog::github::format_error_chain(error.as_ref())
        ))
    })?;

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

#[cfg(test)]
mod tests {
    #[test]
    fn libgit2_supports_https_transport() {
        assert!(
            git2::Version::get().https(),
            "the CI release push requires libgit2 HTTPS support"
        );
    }
}
