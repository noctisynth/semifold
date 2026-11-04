#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod changeset_service;
pub mod config_editor;
pub mod config_management;
pub mod config_sync;
pub mod discovery;
pub mod file_edit_executor;
pub mod git;
pub mod init;
pub mod package_path;
pub mod project;
pub mod publish_plan;
pub mod publisher;
pub mod release;
pub mod release_apply;
pub mod service;
pub mod workflow_output;
pub mod workspace;

pub use changeset_service::{
    ChangesetCreateError, ChangesetCrudError, ChangesetDraft, ChangesetMutationResult,
    ChangesetMutationStatus, ChangesetPackageInput, ChangesetRecord,
};
pub use config_management::{ChannelUpdate, ConfigMutationError, ConfigMutationPlan};
pub use init::{InitFile, InitOptions, InitPlan, InitReport, InitWorkflowTemplates};
pub use project::{Project, ProjectLoadError, ProjectLocation, ProjectLocator};
pub use publish_plan::{PublishOptions, PublishPlan};
pub use publisher::{PublishExecutionError, PublishReport, PublishStatus};
pub use release_apply::{
    ApplyReport, ExecutionMode, PostVersionCommand, PostVersionCommandEvent,
    PostVersionCommandOutcome, PostVersionFailure, ReleaseApplyError, ReleaseApplyPlan,
    ReleaseExecutionOptions,
};
pub use service::{
    AppError, ConfigMutationReport, ConfigSyncOptions, ConfigSyncReport, SemifoldService,
    SystemDependencies,
};
pub use workflow_output::{PublishWorkflowOutput, VersionWorkflowOutput, WorkflowExecutionMode};
