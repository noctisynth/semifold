use std::{
    any::Any,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use clap::Parser;
use rmcp::schemars::JsonSchema;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::common::schema_for_input,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, JsonObject, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use rust_i18n::t;
use semifold_core::{BumpLevel, FileHash, PackageId};
use semifold_engine::{
    AppError, ChangesetCreateError, ChangesetCrudError, ChangesetDraft, ChangesetMutationResult,
    ChangesetMutationStatus, ChangesetPackageInput, ChangesetRecord, ExecutionMode, Project,
    ProjectLoadError, ProjectLocator, SemifoldService, SystemDependencies,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

const MCP_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum McpBumpLevel {
    Patch,
    Minor,
    Major,
}

impl McpBumpLevel {
    const fn into_domain(self) -> BumpLevel {
        match self {
            Self::Patch => BumpLevel::Patch,
            Self::Minor => BumpLevel::Minor,
            Self::Major => BumpLevel::Major,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpChangesetPackageInput {
    pub package: String,
    pub bump: McpBumpLevel,
    #[serde(default)]
    pub tag: Option<String>,
}

impl McpChangesetPackageInput {
    fn into_domain(self) -> ChangesetPackageInput {
        ChangesetPackageInput {
            package: PackageId::new(self.package),
            bump: self.bump.into_domain(),
            tag: self.tag,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetChangesetParams {
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateChangesetParams {
    pub name: String,
    pub packages: Vec<McpChangesetPackageInput>,
    pub summary: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChangesetParams {
    pub id: String,
    pub revision: String,
    pub packages: Vec<McpChangesetPackageInput>,
    pub summary: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteChangesetParams {
    pub id: String,
    pub revision: String,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpChangesetPackage {
    pub package: String,
    pub bump: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct McpChangeset {
    pub id: String,
    pub path: String,
    pub revision: String,
    pub packages: Vec<McpChangesetPackage>,
    pub summary: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetChangesetOutput {
    pub schema_version: u32,
    pub status: String,
    pub changesets: Vec<McpChangeset>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MutateChangesetOutput {
    pub schema_version: u32,
    pub status: String,
    pub changeset: McpChangeset,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct McpToolError {
    schema_version: u32,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    details: BTreeMap<String, Value>,
}

impl McpToolError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: MCP_SCHEMA_VERSION,
            code: code.into(),
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    fn detail(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Parser, Debug)]
pub struct McpCommand {
    #[arg(
        short = 'C',
        long = "project-root",
        visible_alias = "cd",
        help = t!("cli.mcp.flags.project_root")
    )]
    pub project_root: Option<String>,
}

#[derive(Clone)]
pub struct SemifoldMcp {
    locator: ProjectLocator,
    service: Arc<SemifoldService<SystemDependencies>>,
    mutation_lock: Arc<Mutex<()>>,
    mode: ExecutionMode,
    tool_router: ToolRouter<Self>,
}

impl SemifoldMcp {
    fn new(locator: ProjectLocator, mode: ExecutionMode) -> Self {
        let mut tool_router = Self::tool_router();
        localize_tool(
            &mut tool_router,
            "get_changeset",
            "cli.mcp.tools.get_changeset",
        );
        localize_tool(
            &mut tool_router,
            "create_changeset",
            "cli.mcp.tools.create_changeset",
        );
        localize_tool(
            &mut tool_router,
            "update_changeset",
            "cli.mcp.tools.update_changeset",
        );
        localize_tool(
            &mut tool_router,
            "delete_changeset",
            "cli.mcp.tools.delete_changeset",
        );
        Self {
            locator,
            service: Arc::new(SemifoldService::new(SystemDependencies)),
            mutation_lock: Arc::new(Mutex::new(())),
            mode,
            tool_router,
        }
    }

    fn project(&self) -> Result<Project, McpToolError> {
        self.locator.load().map_err(project_error)
    }

    fn guard<T, F>(&self, operation: &'static str, execute: F) -> CallToolResult
    where
        T: Serialize,
        F: FnOnce() -> Result<T, McpToolError>,
    {
        match catch_unwind(AssertUnwindSafe(|| match execute() {
            Ok(output) => structured_success(output),
            Err(error) => structured_error(error),
        })) {
            Ok(result) => result,
            Err(_) => {
                log::error!(
                    "{}",
                    t!("cli.mcp.diagnostics.tool_panicked", operation = operation)
                );
                structured_error(McpToolError::new(
                    "INTERNAL_ERROR",
                    t!("cli.mcp.errors.internal"),
                ))
            }
        }
    }

    fn lock_mutations(&self) -> MutexGuard<'_, ()> {
        match self.mutation_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("{}", t!("cli.mcp.diagnostics.mutation_lock_poisoned"));
                poisoned.into_inner()
            }
        }
    }
}

#[tool_router(router = tool_router)]
impl SemifoldMcp {
    #[tool(
        name = "get_changeset",
        description = "",
        input_schema = safe_input_schema::<GetChangesetParams>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn get_changeset(&self, Parameters(params): Parameters<Value>) -> CallToolResult {
        self.guard("get_changeset", || {
            let params = decode_params::<GetChangesetParams>(params)?;
            let status = if params.id.is_some() {
                "found"
            } else {
                "listed"
            };
            let project = self.project()?;
            let changesets = self
                .service
                .get_changesets(&project, params.id.as_deref())
                .map_err(app_error)?
                .into_iter()
                .map(|record| changeset_output(&project, record))
                .collect();
            Ok(GetChangesetOutput {
                schema_version: MCP_SCHEMA_VERSION,
                status: status.to_string(),
                changesets,
            })
        })
    }

    #[tool(
        name = "create_changeset",
        description = "",
        input_schema = safe_input_schema::<CreateChangesetParams>(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn create_changeset(&self, Parameters(params): Parameters<Value>) -> CallToolResult {
        self.guard("create_changeset", || {
            let params = decode_params::<CreateChangesetParams>(params)?;
            let _mutation = self.lock_mutations();
            let project = self.project()?;
            let result = self
                .service
                .create_changeset_idempotent(
                    &project,
                    ChangesetDraft {
                        name: params.name,
                        packages: params
                            .packages
                            .into_iter()
                            .map(McpChangesetPackageInput::into_domain)
                            .collect(),
                        summary: params.summary,
                    },
                    self.mode,
                )
                .map_err(app_error)?;
            Ok(mutation_output(&project, result))
        })
    }

    #[tool(
        name = "update_changeset",
        description = "",
        input_schema = safe_input_schema::<UpdateChangesetParams>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn update_changeset(&self, Parameters(params): Parameters<Value>) -> CallToolResult {
        self.guard("update_changeset", || {
            let params = decode_params::<UpdateChangesetParams>(params)?;
            let _mutation = self.lock_mutations();
            let project = self.project()?;
            let revision = parse_revision(&params.revision)?;
            let id = params.id;
            let result = self
                .service
                .update_changeset(
                    &project,
                    &id,
                    &revision,
                    ChangesetDraft {
                        name: id.clone(),
                        packages: params
                            .packages
                            .into_iter()
                            .map(McpChangesetPackageInput::into_domain)
                            .collect(),
                        summary: params.summary,
                    },
                    self.mode,
                )
                .map_err(app_error)?;
            Ok(mutation_output(&project, result))
        })
    }

    #[tool(
        name = "delete_changeset",
        description = "",
        input_schema = safe_input_schema::<DeleteChangesetParams>(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn delete_changeset(&self, Parameters(params): Parameters<Value>) -> CallToolResult {
        self.guard("delete_changeset", || {
            let params = decode_params::<DeleteChangesetParams>(params)?;
            let _mutation = self.lock_mutations();
            let project = self.project()?;
            let revision = parse_revision(&params.revision)?;
            let result = self
                .service
                .delete_changeset(&project, &params.id, &revision, self.mode)
                .map_err(app_error)?;
            Ok(mutation_output(&project, result))
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SemifoldMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "semifold",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(t!("cli.mcp.instructions"))
    }
}

fn localize_tool(router: &mut ToolRouter<SemifoldMcp>, tool: &str, key: &str) {
    if let Some(route) = router.map.get_mut(tool) {
        route.attr.description = Some(t!(key).into_owned().into());
    }
}

fn safe_input_schema<T>() -> Arc<JsonObject>
where
    T: JsonSchema + Any,
{
    match catch_unwind(AssertUnwindSafe(schema_for_input::<T>)) {
        Ok(Ok(schema)) => schema,
        Ok(Err(error)) => {
            log::error!("{}", t!("cli.mcp.diagnostics.schema_failed", error = error));
            fallback_input_schema()
        }
        Err(_) => {
            log::error!("{}", t!("cli.mcp.diagnostics.schema_panicked"));
            fallback_input_schema()
        }
    }
}

fn fallback_input_schema() -> Arc<JsonObject> {
    Arc::new(JsonObject::from_iter([(
        "type".to_string(),
        Value::String("object".to_string()),
    )]))
}

fn decode_params<T: DeserializeOwned>(value: Value) -> Result<T, McpToolError> {
    serde_json::from_value(value).map_err(|error| {
        McpToolError::new(
            "INVALID_ARGUMENTS",
            t!("cli.mcp.errors.invalid_arguments", error = error),
        )
    })
}

fn changeset_output(project: &Project, record: ChangesetRecord) -> McpChangeset {
    let path = record
        .path
        .strip_prefix(&project.root)
        .map_or_else(|_| record.path.as_str(), |path| path.as_str())
        .to_string();
    McpChangeset {
        id: record.id.to_string(),
        path,
        revision: record.revision.as_str().to_string(),
        packages: record
            .packages
            .into_iter()
            .map(|package| McpChangesetPackage {
                package: package.package.to_string(),
                bump: package.bump.to_string(),
                tag: package.tag,
            })
            .collect(),
        summary: record.summary,
    }
}

fn mutation_output(project: &Project, result: ChangesetMutationResult) -> MutateChangesetOutput {
    MutateChangesetOutput {
        schema_version: MCP_SCHEMA_VERSION,
        status: mutation_status(result.status).to_string(),
        changeset: changeset_output(project, result.changeset),
    }
}

const fn mutation_status(status: ChangesetMutationStatus) -> &'static str {
    match status {
        ChangesetMutationStatus::Planned => "planned",
        ChangesetMutationStatus::Created => "created",
        ChangesetMutationStatus::Existing => "existing",
        ChangesetMutationStatus::Updated => "updated",
        ChangesetMutationStatus::Deleted => "deleted",
    }
}

fn parse_revision(revision: &str) -> Result<FileHash, McpToolError> {
    FileHash::from_sha256(revision).map_err(|_| {
        McpToolError::new("INVALID_REVISION", t!("cli.mcp.errors.invalid_revision"))
            .detail("revision", revision)
    })
}

fn project_error(error: ProjectLoadError) -> McpToolError {
    let code = match &error {
        ProjectLoadError::RepositoryNotFound => "PROJECT_NOT_FOUND",
        ProjectLoadError::RepositoryOpenFailed { source, .. } => {
            if source.is_permission_denied() {
                "GIT_ACCESS_DENIED"
            } else if source.is_not_found() {
                "PROJECT_NOT_FOUND"
            } else {
                "PROJECT_GIT_ERROR"
            }
        }
        ProjectLoadError::ChangesetDirectoryNotFound | ProjectLoadError::ConfigNotFound => {
            "PROJECT_NOT_INITIALIZED"
        }
        ProjectLoadError::NonUtf8Path { .. } | ProjectLoadError::ConfigInvalid { .. } => {
            "PROJECT_INVALID"
        }
    };
    let message = crate::project_load_error_message_with_fallback(
        &error,
        t!("cli.mcp.errors.project_load_failed", error = error).into_owned(),
    );
    McpToolError::new(code, message)
}

fn app_error(error: AppError) -> McpToolError {
    match error {
        AppError::ChangesetCrud(error) => changeset_error(error),
        AppError::ChangesetCreate(error) => create_error(error),
        error => {
            log::error!("{}", t!("cli.mcp.diagnostics.app_error", error = error));
            McpToolError::new("INTERNAL_ERROR", t!("cli.mcp.errors.internal"))
        }
    }
}

fn changeset_error(error: ChangesetCrudError) -> McpToolError {
    match error {
        ChangesetCrudError::Invalid(error) => create_error(error),
        ChangesetCrudError::InvalidId { id } => McpToolError::new(
            "INVALID_CHANGESET_ID",
            t!("cli.mcp.errors.invalid_id", id = id),
        )
        .detail("id", id),
        ChangesetCrudError::IdMismatch { target, draft } => McpToolError::new(
            "CHANGESET_ID_MISMATCH",
            t!("cli.mcp.errors.id_mismatch", target = target, draft = draft),
        )
        .detail("id", target)
        .detail("draftId", draft),
        ChangesetCrudError::NotFound { id } => McpToolError::new(
            "CHANGESET_NOT_FOUND",
            t!("cli.mcp.errors.not_found", id = id.as_str()),
        )
        .detail("id", id.as_str()),
        ChangesetCrudError::Conflict { id, actual } => McpToolError::new(
            "CHANGESET_CONFLICT",
            t!("cli.mcp.errors.conflict", id = id.as_str()),
        )
        .detail("id", id.as_str())
        .detail("actualRevision", actual.as_str()),
        ChangesetCrudError::RevisionMismatch {
            id,
            expected,
            actual,
        } => McpToolError::new(
            "CHANGESET_REVISION_MISMATCH",
            t!("cli.mcp.errors.revision_mismatch", id = id.as_str()),
        )
        .detail("id", id.as_str())
        .detail("expectedRevision", expected.as_str())
        .detail("actualRevision", actual.as_str()),
        error => {
            log::error!("{}", t!("cli.mcp.diagnostics.io_error", error = error));
            McpToolError::new("CHANGESET_IO_ERROR", t!("cli.mcp.errors.io"))
        }
    }
}

fn create_error(error: ChangesetCreateError) -> McpToolError {
    let (code, message) = match &error {
        ChangesetCreateError::EmptyName => ("EMPTY_NAME", t!("cli.mcp.errors.empty_name")),
        ChangesetCreateError::AlreadyExists { name } => (
            "CHANGESET_CONFLICT",
            t!("cli.mcp.errors.conflict", id = name),
        ),
        ChangesetCreateError::EmptySummary => ("EMPTY_SUMMARY", t!("cli.mcp.errors.empty_summary")),
        ChangesetCreateError::EmptyPackages => {
            ("EMPTY_PACKAGES", t!("cli.mcp.errors.empty_packages"))
        }
        ChangesetCreateError::PackageNotFound { package } => (
            "PACKAGE_NOT_FOUND",
            t!(
                "cli.mcp.errors.package_not_found",
                package = package.as_str()
            ),
        ),
        ChangesetCreateError::DuplicatePackage { package } => (
            "DUPLICATE_PACKAGE",
            t!(
                "cli.mcp.errors.duplicate_package",
                package = package.as_str()
            ),
        ),
        ChangesetCreateError::UnchangedPackage { package } => (
            "UNCHANGED_PACKAGE",
            t!(
                "cli.mcp.errors.unchanged_package",
                package = package.as_str()
            ),
        ),
        ChangesetCreateError::TagNotFound { tag } => (
            "TAG_NOT_FOUND",
            t!("cli.mcp.errors.tag_not_found", tag = tag),
        ),
        ChangesetCreateError::Write(error) => {
            log::error!("{}", t!("cli.mcp.diagnostics.render_error", error = error));
            ("CHANGESET_IO_ERROR", t!("cli.mcp.errors.io"))
        }
    };
    McpToolError::new(code, message)
}

fn structured_success<T: Serialize>(output: T) -> CallToolResult {
    match serde_json::to_value(output) {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => {
            log::error!(
                "{}",
                t!("cli.mcp.diagnostics.result_serialize_failed", error = error)
            );
            structured_error(McpToolError::new(
                "INTERNAL_ERROR",
                t!("cli.mcp.errors.internal"),
            ))
        }
    }
}

fn structured_error(error: McpToolError) -> CallToolResult {
    match serde_json::to_value(&error) {
        Ok(value) => CallToolResult::structured_error(value),
        Err(serialization_error) => {
            log::error!(
                "{}",
                t!(
                    "cli.mcp.diagnostics.error_serialize_failed",
                    error = serialization_error
                )
            );
            CallToolResult::error(vec![ContentBlock::text(t!("cli.mcp.errors.internal"))])
        }
    }
}

pub async fn run_mcp(opts: &McpCommand, dry_run: bool) -> anyhow::Result<()> {
    let start = opts
        .project_root
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let changeset_dir = std::env::var_os("CHANGESET_PATH").map(PathBuf::from);
    let mode = if dry_run {
        ExecutionMode::DryRun
    } else {
        ExecutionMode::Apply
    };
    let server = SemifoldMcp::new(ProjectLocator::new(start, changeset_dir), mode);
    let service = server.serve(rmcp::transport::io::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    fn repository(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock in tests must be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "semifold-mcp-{name}-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("MCP fixture directory must be created");
        gix::init(&root).expect("MCP fixture repository must be initialized");
        root
    }

    fn initialize_project(root: &Path) {
        let changeset_dir = root.join(".changes");
        fs::create_dir_all(&changeset_dir).expect("changeset fixture directory must be created");
        fs::write(
            changeset_dir.join("config.toml"),
            "[branches]\nbase = \"main\"\nrelease = \"release\"\n\n[tags]\nfix = \"Bug Fixes\"\n\n[packages.app]\npath = \".\"\nresolver = \"rust\"\n\n[resolver.rust.pre-check]\ntype = \"http\"\nurl = \"\"\n",
        )
        .expect("MCP fixture configuration must be written");
    }

    fn server(root: &Path, mode: ExecutionMode) -> SemifoldMcp {
        SemifoldMcp::new(ProjectLocator::new(root.to_path_buf(), None), mode)
    }

    fn package() -> McpChangesetPackageInput {
        McpChangesetPackageInput {
            package: "app".to_string(),
            bump: McpBumpLevel::Patch,
            tag: Some("fix".to_string()),
        }
    }

    fn structured(result: &CallToolResult) -> &Value {
        result
            .structured_content
            .as_ref()
            .expect("MCP result must contain structured content")
    }

    fn parameters<T: Serialize>(value: T) -> Parameters<Value> {
        Parameters(serde_json::to_value(value).expect("MCP fixture parameters must serialize"))
    }

    #[test]
    fn project_errors_are_tool_errors_and_the_same_server_recovers_after_init() {
        let root = repository("lazy-project");
        let server = server(&root, ExecutionMode::Apply);

        let before_init = server.get_changeset(parameters(GetChangesetParams { id: None }));
        assert_eq!(before_init.is_error, Some(true));
        assert_eq!(
            structured(&before_init)["code"],
            Value::String("PROJECT_NOT_INITIALIZED".to_string())
        );

        initialize_project(&root);
        let after_init = server.get_changeset(parameters(GetChangesetParams { id: None }));
        assert_eq!(after_init.is_error, Some(false), "{after_init:?}");
        assert_eq!(structured(&after_init)["status"], "listed");
        assert_eq!(
            structured(&after_init)["changesets"],
            Value::Array(Vec::new())
        );

        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }

    #[test]
    fn invalid_toml_project_error_includes_the_migration_hint() {
        let root = repository("legacy-config");
        initialize_project(&root);
        let config_path = root.join(".changes/config.toml");
        let legacy = fs::read_to_string(&config_path)
            .expect("MCP fixture configuration must be readable")
            .replace("type = \"http\"\n", "");
        fs::write(&config_path, legacy).expect("legacy MCP fixture must be written");
        let server = server(&root, ExecutionMode::Apply);

        let result = server.get_changeset(parameters(GetChangesetParams { id: None }));

        assert_eq!(result.is_error, Some(true));
        assert_eq!(structured(&result)["code"], "PROJECT_INVALID");
        let message = structured(&result)["message"]
            .as_str()
            .expect("MCP project error must contain a message");
        assert!(message.starts_with("Invalid config"), "{message}");
        assert!(message.contains("missing field `type`"), "{message}");
        assert!(
            message.contains("\n✘ The configuration may use"),
            "{message}"
        );
        assert!(message.contains("smif config migrate"), "{message}");
        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }

    #[test]
    fn server_advertises_only_the_tools_it_implements() {
        let root = repository("capabilities");
        let server = server(&root, ExecutionMode::Apply);
        let info = server.get_info();
        let tools = server.tool_router.list_all();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(info.server_info.name, "semifold");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_none());
        assert!(info.capabilities.prompts.is_none());
        assert!(info.capabilities.logging.is_none());
        assert_eq!(
            names,
            vec![
                "create_changeset",
                "delete_changeset",
                "get_changeset",
                "update_changeset"
            ]
        );
        assert!(tools.iter().all(|tool| tool.description.is_some()));
        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }

    #[test]
    fn crud_returns_structured_conflicts_and_continues_serving() {
        let root = repository("crud");
        initialize_project(&root);
        let server = server(&root, ExecutionMode::Apply);

        let created = server.create_changeset(parameters(CreateChangesetParams {
            name: "mcp-crud".to_string(),
            packages: vec![package()],
            summary: "Exercise MCP CRUD".to_string(),
        }));
        assert_eq!(created.is_error, Some(false), "{created:?}");
        assert_eq!(structured(&created)["status"], "created");
        let revision = structured(&created)["changeset"]["revision"]
            .as_str()
            .expect("created changeset must expose its revision")
            .to_string();

        let stale_update = server.update_changeset(parameters(UpdateChangesetParams {
            id: "mcp-crud".to_string(),
            revision: "0".repeat(64),
            packages: vec![package()],
            summary: "Rejected replacement".to_string(),
        }));
        assert_eq!(stale_update.is_error, Some(true));
        assert_eq!(
            structured(&stale_update)["code"],
            "CHANGESET_REVISION_MISMATCH"
        );

        let updated = server.update_changeset(parameters(UpdateChangesetParams {
            id: "mcp-crud".to_string(),
            revision,
            packages: vec![package()],
            summary: "Accepted replacement".to_string(),
        }));
        assert_eq!(updated.is_error, Some(false));
        assert_eq!(structured(&updated)["status"], "updated");
        let updated_revision = structured(&updated)["changeset"]["revision"]
            .as_str()
            .expect("updated changeset must expose its revision")
            .to_string();

        let deleted = server.delete_changeset(parameters(DeleteChangesetParams {
            id: "mcp-crud".to_string(),
            revision: updated_revision,
        }));
        assert_eq!(deleted.is_error, Some(false));
        assert_eq!(structured(&deleted)["status"], "deleted");

        let after_delete = server.get_changeset(parameters(GetChangesetParams { id: None }));
        assert_eq!(after_delete.is_error, Some(false));
        assert_eq!(
            structured(&after_delete)["changesets"],
            Value::Array(Vec::new())
        );
        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }

    #[test]
    fn dry_run_reports_planned_without_writing() {
        let root = repository("dry-run");
        initialize_project(&root);
        let server = server(&root, ExecutionMode::DryRun);

        let result = server.create_changeset(parameters(CreateChangesetParams {
            name: "planned".to_string(),
            packages: vec![package()],
            summary: "Plan MCP creation".to_string(),
        }));

        assert_eq!(result.is_error, Some(false), "{result:?}");
        assert_eq!(structured(&result)["status"], "planned");
        assert!(!root.join(".changes/planned.md").exists());
        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }

    #[test]
    fn caught_panics_become_internal_errors_and_do_not_poison_the_server() {
        let root = repository("panic-isolation");
        initialize_project(&root);
        let server = server(&root, ExecutionMode::Apply);

        let panic_result = server.guard::<Value, _>("test", || {
            panic!("intentional MCP panic fixture");
        });
        assert_eq!(panic_result.is_error, Some(true));
        assert_eq!(structured(&panic_result)["code"], "INTERNAL_ERROR");

        let next_result = server.get_changeset(parameters(GetChangesetParams { id: None }));
        assert_eq!(next_result.is_error, Some(false), "{next_result:?}");
        fs::remove_dir_all(root).expect("MCP fixture must be removed");
    }
}
