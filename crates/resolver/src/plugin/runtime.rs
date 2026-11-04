use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use boa_engine::module::IdleModuleLoader;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::{IntegrityLevel, ObjectInitializer};
use boa_engine::{
    Context, Finalize, JsArgs, JsData, JsError, JsResult, JsString, JsValue, Module,
    NativeFunction, Source, Trace, js_string,
};
use boa_runtime::fetch::Fetcher;
use boa_runtime::fetch::request::JsRequest;
use boa_runtime::fetch::response::JsResponse;
use semifold_core::EcosystemId;

use super::file::{
    DenyPluginFileClient, MAX_FILE_BYTES, MAX_OPERATION_FILE_BYTES, MAX_OPERATION_PATHS,
    PluginFileClient, PluginFileError, matches_pattern, validate_file_path, validate_pattern_path,
};
#[cfg(test)]
use super::http::PluginHttpFuture;
use super::http::{
    BudgetedPluginHttpClient, BudgetedPluginHttpTransport, DenyPluginHttpClient, PluginHttpClient,
    PluginHttpLimits, PluginHttpOrigin, PluginHttpRequest, PluginHttpResponse, PluginHttpTransport,
    ScopedPluginHttpClient,
};
use super::protocol::{PluginMetadataV1, PluginProtocolError, PluginRequestV1, PluginResponseV1};

pub(crate) const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_LOOP_ITERATIONS: u64 = 10_000_000;
const MAX_RECURSION_DEPTH: usize = 256;
const MAX_VM_STACK_VALUES: usize = 10_240;

#[derive(Clone, Trace, Finalize, JsData)]
struct BoaFetcher {
    #[unsafe_ignore_trace]
    client: Arc<dyn PluginHttpClient>,
}

#[derive(Clone)]
enum BoaHttpBackend {
    Client(Arc<dyn PluginHttpClient>),
    Transport {
        allowed_origins: BTreeSet<PluginHttpOrigin>,
        transport: Arc<dyn PluginHttpTransport>,
    },
}

impl fmt::Debug for BoaHttpBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(_) => formatter.write_str("BoaHttpBackend::Client(..)"),
            Self::Transport {
                allowed_origins, ..
            } => formatter
                .debug_struct("BoaHttpBackend::Transport")
                .field("allowed_origins", allowed_origins)
                .finish_non_exhaustive(),
        }
    }
}

impl fmt::Debug for BoaFetcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("BoaFetcher").finish_non_exhaustive()
    }
}

impl Fetcher for BoaFetcher {
    async fn fetch(
        self: Rc<Self>,
        request: JsRequest,
        _signal: Option<boa_engine::JsObject>,
        _context: &std::cell::RefCell<&mut Context>,
    ) -> JsResult<JsResponse> {
        let request = request.into_inner();
        let plugin_request = PluginHttpRequest {
            method: request.method().to_string(),
            url: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
                .collect(),
            body: request.body().clone(),
        };
        let response = self
            .client
            .send(plugin_request)
            .await
            .map_err(JsError::from_rust)?;
        let PluginHttpResponse {
            url,
            status,
            headers,
            body,
        } = response;
        let mut builder = http::Response::builder().status(status);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        let response = builder.body(body).map_err(JsError::from_rust)?;
        Ok(JsResponse::basic(JsString::from(url), response))
    }
}

#[derive(Clone, Debug, Default)]
struct PluginFileBudget {
    returned_paths: usize,
    returned_bytes: usize,
}

#[derive(Clone, Trace, Finalize, JsData)]
struct BoaFileHost {
    #[unsafe_ignore_trace]
    client: Arc<dyn PluginFileClient>,
    #[unsafe_ignore_trace]
    budget: Arc<Mutex<PluginFileBudget>>,
    max_file_bytes: usize,
    max_operation_file_bytes: usize,
    max_operation_paths: usize,
}

impl fmt::Debug for BoaFileHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoaFileHost")
            .finish_non_exhaustive()
    }
}

impl BoaFileHost {
    fn charge_paths(&self, count: usize) -> Result<(), PluginFileError> {
        let mut budget = self
            .budget
            .lock()
            .map_err(|_| PluginFileError::BudgetStateUnavailable)?;
        let actual = budget.returned_paths.saturating_add(count);
        if actual > self.max_operation_paths {
            return Err(PluginFileError::TooManyPaths {
                actual,
                maximum: self.max_operation_paths,
            });
        }
        budget.returned_paths = actual;
        Ok(())
    }

    fn charge_bytes(&self, path: &str, count: usize) -> Result<(), PluginFileError> {
        if count > self.max_file_bytes {
            return Err(PluginFileError::FileTooLarge {
                path: path.to_owned(),
                actual: count,
                maximum: self.max_file_bytes,
            });
        }
        let mut budget = self
            .budget
            .lock()
            .map_err(|_| PluginFileError::BudgetStateUnavailable)?;
        let actual = budget.returned_bytes.saturating_add(count);
        if actual > self.max_operation_file_bytes {
            return Err(PluginFileError::OperationBytesExceeded {
                actual,
                maximum: self.max_operation_file_bytes,
            });
        }
        budget.returned_bytes = actual;
        Ok(())
    }
}

async fn boa_list_files(
    _this: &JsValue,
    arguments: &[JsValue],
    context: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (pattern, host) = {
        let mut context = context.borrow_mut();
        let pattern = string_argument(arguments, 0, "listFiles", &mut context)?;
        validate_pattern_path(&pattern).map_err(JsError::from_rust)?;
        let host = context
            .get_data::<BoaFileHost>()
            .cloned()
            .ok_or(PluginFileError::HostUnavailable)
            .map_err(JsError::from_rust)?;
        (pattern, host)
    };

    let mut paths = host
        .client
        .list_files(&pattern)
        .await
        .map_err(JsError::from_rust)?;
    for path in &paths {
        validate_file_path(path).map_err(JsError::from_rust)?;
        if !matches_pattern(&pattern, path).map_err(JsError::from_rust)? {
            return Err(JsError::from_rust(
                PluginFileError::ReturnedPathDoesNotMatch {
                    path: path.clone(),
                    pattern,
                },
            ));
        }
    }
    paths.sort();
    paths.dedup();
    host.charge_paths(paths.len()).map_err(JsError::from_rust)?;
    let value =
        serde_json::Value::Array(paths.into_iter().map(serde_json::Value::String).collect());
    JsValue::from_json(&value, &mut context.borrow_mut())
}

async fn boa_read_text(
    _this: &JsValue,
    arguments: &[JsValue],
    context: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (path, host) = {
        let mut context = context.borrow_mut();
        let path = string_argument(arguments, 0, "readText", &mut context)?;
        validate_file_path(&path).map_err(JsError::from_rust)?;
        let host = context
            .get_data::<BoaFileHost>()
            .cloned()
            .ok_or(PluginFileError::HostUnavailable)
            .map_err(JsError::from_rust)?;
        (path, host)
    };

    let content = host
        .client
        .read_text(&path)
        .await
        .map_err(JsError::from_rust)?;
    host.charge_bytes(&path, content.len())
        .map_err(JsError::from_rust)?;
    Ok(JsString::from(content).into())
}

fn string_argument(
    arguments: &[JsValue],
    index: usize,
    method: &'static str,
    context: &mut Context,
) -> JsResult<String> {
    let value = arguments.get_or_undefined(index);
    if !value.is_string() {
        return Err(JsError::from_rust(PluginFileError::InvalidArgument {
            method,
        }));
    }
    value
        .to_string(context)?
        .to_std_string()
        .map_err(JsError::from_rust)
}

#[derive(Clone, Copy, Debug)]
struct BoaLimits {
    max_source_bytes: usize,
    max_request_bytes: usize,
    max_response_bytes: usize,
    max_loop_iterations: u64,
    max_recursion_depth: usize,
    max_vm_stack_values: usize,
    max_file_bytes: usize,
    max_operation_file_bytes: usize,
    max_operation_paths: usize,
    http: PluginHttpLimits,
}

impl Default for BoaLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: MAX_SOURCE_BYTES,
            max_request_bytes: MAX_REQUEST_BYTES,
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_loop_iterations: MAX_LOOP_ITERATIONS,
            max_recursion_depth: MAX_RECURSION_DEPTH,
            max_vm_stack_values: MAX_VM_STACK_VALUES,
            max_file_bytes: MAX_FILE_BYTES,
            max_operation_file_bytes: MAX_OPERATION_FILE_BYTES,
            max_operation_paths: MAX_OPERATION_PATHS,
            http: PluginHttpLimits::default(),
        }
    }
}

/// Embedded Boa host for the schema-versioned Semifold plugin protocol.
#[derive(Clone)]
pub struct BoaPluginRuntime {
    http_backend: BoaHttpBackend,
    file_client: Arc<dyn PluginFileClient>,
    limits: BoaLimits,
}

impl fmt::Debug for BoaPluginRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoaPluginRuntime")
            .finish_non_exhaustive()
    }
}

impl Default for BoaPluginRuntime {
    fn default() -> Self {
        Self::new(DenyPluginHttpClient)
    }
}

impl BoaPluginRuntime {
    #[must_use]
    pub fn new(http_client: impl PluginHttpClient) -> Self {
        Self {
            http_backend: BoaHttpBackend::Client(Arc::new(http_client)),
            file_client: Arc::new(DenyPluginFileClient),
            limits: BoaLimits::default(),
        }
    }

    #[must_use]
    pub fn with_file_client(mut self, file_client: impl PluginFileClient) -> Self {
        self.file_client = Arc::new(file_client);
        self
    }

    #[must_use]
    pub fn with_http_transport(
        mut self,
        allowed_origins: impl IntoIterator<Item = PluginHttpOrigin>,
        transport: impl PluginHttpTransport,
    ) -> Self {
        self.http_backend = BoaHttpBackend::Transport {
            allowed_origins: allowed_origins.into_iter().collect(),
            transport: Arc::new(transport),
        };
        self
    }

    pub(crate) fn with_shared_http_transport(
        mut self,
        allowed_origins: BTreeSet<PluginHttpOrigin>,
        transport: Arc<dyn PluginHttpTransport>,
    ) -> Self {
        self.http_backend = BoaHttpBackend::Transport {
            allowed_origins,
            transport,
        };
        self
    }

    pub(crate) fn with_shared_http_client(mut self, client: Arc<dyn PluginHttpClient>) -> Self {
        self.http_backend = BoaHttpBackend::Client(client);
        self
    }

    /// Loads and validates the named `metadata` export in a fresh Boa context.
    pub fn metadata(&self, source: &str) -> Result<PluginMetadataV1, PluginRuntimeError> {
        let (mut context, module) = self.load_module(source, Arc::new(DenyPluginHttpClient))?;
        let value = module
            .get_value(js_string!("metadata"), &mut context)
            .map_err(|error| PluginRuntimeError::MissingMetadata(error.to_string()))?;
        let value = value
            .to_json(&mut context)
            .map_err(|error| PluginRuntimeError::InvalidMetadata(error.to_string()))?
            .ok_or(PluginRuntimeError::MetadataNotJson)?;
        let metadata = serde_json::from_value::<PluginMetadataV1>(value)
            .map_err(PluginRuntimeError::MetadataDeserialization)?;
        metadata.validate()?;
        for pattern in &metadata.read_patterns {
            validate_pattern_path(pattern)?;
        }
        Ok(metadata)
    }

    /// Executes the module's default export with a validated protocol request.
    pub fn execute(
        &self,
        source: &str,
        request: &PluginRequestV1,
        plugin: &EcosystemId,
    ) -> Result<PluginResponseV1, PluginRuntimeError> {
        request.validate()?;
        let request_json =
            serde_json::to_value(request).map_err(PluginRuntimeError::RequestSerialization)?;
        let request_bytes =
            serde_json::to_vec(&request_json).map_err(PluginRuntimeError::RequestSerialization)?;
        if request_bytes.len() > self.limits.max_request_bytes {
            return Err(PluginRuntimeError::RequestTooLarge {
                actual: request_bytes.len(),
                maximum: self.limits.max_request_bytes,
            });
        }

        let (mut context, module) = self.load_module(source, self.operation_http_client())?;
        let entrypoint = module
            .get_value(js_string!("default"), &mut context)
            .map_err(|error| PluginRuntimeError::MissingEntrypoint(error.to_string()))?;
        let Some(entrypoint) = entrypoint.as_callable() else {
            return Err(PluginRuntimeError::EntrypointNotCallable);
        };
        let request_value = JsValue::from_json(&request_json, &mut context)
            .map_err(|error| PluginRuntimeError::RequestConversion(error.to_string()))?;
        let host_value = self.file_host(&mut context)?;
        let result = entrypoint
            .call(
                &JsValue::undefined(),
                &[request_value, host_value],
                &mut context,
            )
            .map_err(|error| PluginRuntimeError::EntrypointInvocation(error.to_string()))?;
        let result = JsPromise::resolve(result, &mut context)
            .map_err(|error| PluginRuntimeError::EntrypointInvocation(error.to_string()))?
            .await_blocking(&mut context)
            .map_err(|error| PluginRuntimeError::EntrypointInvocation(error.to_string()))?;
        let result = result
            .to_json(&mut context)
            .map_err(|error| PluginRuntimeError::ResponseConversion(error.to_string()))?
            .ok_or(PluginRuntimeError::ResponseNotJson)?;
        let response_bytes =
            serde_json::to_vec(&result).map_err(PluginRuntimeError::ResponseSerialization)?;
        if response_bytes.len() > self.limits.max_response_bytes {
            return Err(PluginRuntimeError::ResponseTooLarge {
                actual: response_bytes.len(),
                maximum: self.limits.max_response_bytes,
            });
        }
        let response = serde_json::from_value::<PluginResponseV1>(result)
            .map_err(PluginRuntimeError::ResponseDeserialization)?;
        response.validate_for(request, plugin)?;
        Ok(response)
    }

    fn file_host(&self, context: &mut Context) -> Result<JsValue, PluginRuntimeError> {
        context.insert_data(BoaFileHost {
            client: self.file_client.clone(),
            budget: Arc::new(Mutex::new(PluginFileBudget::default())),
            max_file_bytes: self.limits.max_file_bytes,
            max_operation_file_bytes: self.limits.max_operation_file_bytes,
            max_operation_paths: self.limits.max_operation_paths,
        });
        let host = ObjectInitializer::new(context)
            .function(
                NativeFunction::from_async_fn(boa_list_files),
                js_string!("listFiles"),
                1,
            )
            .function(
                NativeFunction::from_async_fn(boa_read_text),
                js_string!("readText"),
                1,
            )
            .build();
        let frozen = host
            .set_integrity_level(IntegrityLevel::Frozen, context)
            .map_err(|error| PluginRuntimeError::RuntimeInitialization(error.to_string()))?;
        if !frozen {
            return Err(PluginRuntimeError::HostInitialization);
        }
        Ok(host.into())
    }

    fn operation_http_client(&self) -> Arc<dyn PluginHttpClient> {
        match &self.http_backend {
            BoaHttpBackend::Client(client) => Arc::new(BudgetedPluginHttpClient::new(
                client.clone(),
                self.limits.http,
            )),
            BoaHttpBackend::Transport {
                allowed_origins,
                transport,
            } => {
                let transport: Arc<dyn PluginHttpTransport> = Arc::new(
                    BudgetedPluginHttpTransport::new(transport.clone(), self.limits.http),
                );
                Arc::new(ScopedPluginHttpClient::from_shared(
                    allowed_origins.clone(),
                    transport,
                ))
            }
        }
    }

    fn load_module(
        &self,
        source: &str,
        http_client: Arc<dyn PluginHttpClient>,
    ) -> Result<(Context, Module), PluginRuntimeError> {
        if source.len() > self.limits.max_source_bytes {
            return Err(PluginRuntimeError::SourceTooLarge {
                actual: source.len(),
                maximum: self.limits.max_source_bytes,
            });
        }
        let mut context = Context::builder()
            .module_loader(Rc::new(IdleModuleLoader))
            .build()
            .map_err(|error| PluginRuntimeError::RuntimeInitialization(error.to_string()))?;
        {
            let limits = context.runtime_limits_mut();
            limits.set_loop_iteration_limit(self.limits.max_loop_iterations);
            limits.set_recursion_limit(self.limits.max_recursion_depth);
            limits.set_stack_size_limit(self.limits.max_vm_stack_values);
        }
        boa_runtime::fetch::register(
            BoaFetcher {
                client: http_client,
            },
            None,
            &mut context,
        )
        .map_err(|error| PluginRuntimeError::RuntimeInitialization(error.to_string()))?;
        boa_runtime::url::Url::register(None, &mut context)
            .map_err(|error| PluginRuntimeError::RuntimeInitialization(error.to_string()))?;

        let module = Module::parse(Source::from_bytes(source.as_bytes()), None, &mut context)
            .map_err(|error| PluginRuntimeError::ModuleParsing(error.to_string()))?;
        module
            .load_link_evaluate(&mut context)
            .await_blocking(&mut context)
            .map_err(|error| PluginRuntimeError::ModuleEvaluation(error.to_string()))?;
        Ok((context, module))
    }

    #[cfg(test)]
    fn with_loop_limit(mut self, maximum: u64) -> Self {
        self.limits.max_loop_iterations = maximum;
        self
    }

    #[cfg(test)]
    fn with_file_limits(
        mut self,
        max_file_bytes: usize,
        max_operation_file_bytes: usize,
        max_operation_paths: usize,
    ) -> Self {
        self.limits.max_file_bytes = max_file_bytes;
        self.limits.max_operation_file_bytes = max_operation_file_bytes;
        self.limits.max_operation_paths = max_operation_paths;
        self
    }

    #[cfg(test)]
    fn with_http_limits(mut self, limits: PluginHttpLimits) -> Self {
        self.limits.http = limits;
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginRuntimeError {
    #[error("plugin source contains {actual} bytes; maximum is {maximum}")]
    SourceTooLarge { actual: usize, maximum: usize },
    #[error("plugin request contains {actual} bytes; maximum is {maximum}")]
    RequestTooLarge { actual: usize, maximum: usize },
    #[error("plugin response contains {actual} bytes; maximum is {maximum}")]
    ResponseTooLarge { actual: usize, maximum: usize },
    #[error("failed to initialize Boa plugin runtime: {0}")]
    RuntimeInitialization(String),
    #[error("failed to freeze the Boa plugin capability host")]
    HostInitialization,
    #[error("failed to parse plugin module: {0}")]
    ModuleParsing(String),
    #[error("failed to evaluate plugin module: {0}")]
    ModuleEvaluation(String),
    #[error("plugin module does not export metadata: {0}")]
    MissingMetadata(String),
    #[error("plugin metadata is not JSON-compatible: {0}")]
    InvalidMetadata(String),
    #[error("plugin metadata resolved to undefined")]
    MetadataNotJson,
    #[error("failed to deserialize plugin metadata: {0}")]
    MetadataDeserialization(#[source] serde_json::Error),
    #[error("plugin module does not have a default export: {0}")]
    MissingEntrypoint(String),
    #[error("plugin module default export must be a function")]
    EntrypointNotCallable,
    #[error("failed to serialize plugin request: {0}")]
    RequestSerialization(#[source] serde_json::Error),
    #[error("failed to convert plugin request to JavaScript: {0}")]
    RequestConversion(String),
    #[error("plugin entrypoint failed: {0}")]
    EntrypointInvocation(String),
    #[error("plugin response is not JSON-compatible: {0}")]
    ResponseConversion(String),
    #[error("plugin response resolved to undefined")]
    ResponseNotJson,
    #[error("failed to serialize plugin response: {0}")]
    ResponseSerialization(#[source] serde_json::Error),
    #[error("failed to deserialize plugin response: {0}")]
    ResponseDeserialization(#[source] serde_json::Error),
    #[error(transparent)]
    Protocol(#[from] PluginProtocolError),
    #[error(transparent)]
    FileCapability(#[from] PluginFileError),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::plugin::protocol::{PluginCallV1, PluginDiscoverInputV1};

    const PLUGIN_ID: &str = "com.example.engine";

    fn plugin_id() -> EcosystemId {
        EcosystemId::new(PLUGIN_ID).unwrap()
    }

    fn request() -> PluginRequestV1 {
        PluginRequestV1::new(PluginCallV1::Discover(PluginDiscoverInputV1 {
            project_root: ".".to_owned(),
        }))
    }

    fn module_source(body: &str) -> String {
        format!(
            r#"
            export const metadata = {{
                "schema-version": 1,
                ecosystem: "{PLUGIN_ID}",
                "plugin-version": "1.0.0",
                operations: ["discover", "inspect", "plan-edits"]
            }};
            export default {body};
            "#
        )
    }

    #[test]
    fn loads_metadata_and_executes_async_default_export() {
        let source = module_source(
            r#"async function(request) {
                await Promise.resolve();
                return {
                    "schema-version": request["schema-version"],
                    diagnostics: [],
                    status: "success",
                    output: {
                        operation: request.operation,
                        output: { packages: [] }
                    }
                };
            }"#,
        );
        let runtime = BoaPluginRuntime::default();

        let metadata = runtime.metadata(&source).unwrap();
        assert_eq!(metadata.ecosystem, plugin_id());
        let response = runtime.execute(&source, &request(), &plugin_id()).unwrap();
        assert_eq!(response.diagnostics, Vec::new());
    }

    #[test]
    fn rejects_imports_in_single_file_modules() {
        let source = format!(
            "import value from './other.js'; {}",
            module_source("function() { return value; }")
        );
        assert!(matches!(
            BoaPluginRuntime::default().metadata(&source),
            Err(PluginRuntimeError::ModuleEvaluation(message))
                if message.contains("module resolution is disabled")
        ));
    }

    #[test]
    fn stops_plugins_that_exceed_the_loop_budget() {
        let source =
            module_source("function() { for (let i = 0; i < 20; i += 1) {} return undefined; }");
        let runtime = BoaPluginRuntime::default().with_loop_limit(10);
        let error = runtime
            .execute(&source, &request(), &plugin_id())
            .unwrap_err();
        assert!(
            matches!(
                &error,
                PluginRuntimeError::EntrypointInvocation(message)
                    if message.contains("RuntimeLimitError")
                        && message.contains("maximum number of iteration loops")
            ),
            "{error:?}"
        );
    }

    #[derive(Clone, Debug)]
    struct RecordingHttpClient {
        requests: Arc<Mutex<Vec<PluginHttpRequest>>>,
    }

    impl PluginHttpClient for RecordingHttpClient {
        fn send(&self, request: PluginHttpRequest) -> PluginHttpFuture<'_> {
            Box::pin(async move {
                self.requests.lock().unwrap().push(request.clone());
                let response = json!({
                    "schema-version": 1,
                    "diagnostics": [],
                    "status": "success",
                    "output": {
                        "operation": "discover",
                        "output": { "packages": [] }
                    }
                });
                Ok(PluginHttpResponse {
                    url: request.url,
                    status: 200,
                    headers: vec![("content-type".to_owned(), b"application/json".to_vec())],
                    body: serde_json::to_vec(&response).unwrap(),
                })
            })
        }
    }

    #[derive(Clone, Debug)]
    struct RecordingHttpTransport {
        requests: Arc<Mutex<Vec<PluginHttpRequest>>>,
    }

    impl PluginHttpTransport for RecordingHttpTransport {
        fn send_once(&self, request: PluginHttpRequest) -> PluginHttpFuture<'_> {
            Box::pin(async move {
                self.requests.lock().unwrap().push(request.clone());
                let response = json!({
                    "schema-version": 1,
                    "diagnostics": [],
                    "status": "success",
                    "output": {
                        "operation": "discover",
                        "output": { "packages": [] }
                    }
                });
                Ok(PluginHttpResponse {
                    url: request.url,
                    status: 200,
                    headers: vec![("content-type".to_owned(), b"application/json".to_vec())],
                    body: serde_json::to_vec(&response).unwrap(),
                })
            })
        }
    }

    #[derive(Clone, Debug)]
    struct RecordingFileClient {
        paths: Vec<String>,
        content: String,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl PluginFileClient for RecordingFileClient {
        fn list_files(
            &self,
            pattern: &str,
        ) -> super::super::file::PluginFileFuture<'_, Vec<String>> {
            let pattern = pattern.to_owned();
            Box::pin(async move {
                self.calls.lock().unwrap().push(format!("list:{pattern}"));
                Ok(self.paths.clone())
            })
        }

        fn read_text(&self, path: &str) -> super::super::file::PluginFileFuture<'_, String> {
            let path = path.to_owned();
            Box::pin(async move {
                self.calls.lock().unwrap().push(format!("read:{path}"));
                Ok(self.content.clone())
            })
        }
    }

    #[test]
    fn exposes_a_frozen_async_file_host_with_deterministic_listings() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = BoaPluginRuntime::default().with_file_client(RecordingFileClient {
            paths: vec![
                "packages/zeta/package.json".to_owned(),
                "packages/alpha/package.json".to_owned(),
                "packages/alpha/package.json".to_owned(),
            ],
            content: "alpha".to_owned(),
            calls: calls.clone(),
        });
        let source = module_source(
            r#"async function(request, host) {
                if (!Object.isFrozen(host)) {
                    throw new Error("host must be frozen");
                }
                const files = await host.listFiles("packages/**/package.json");
                if (files.join(",") !== "packages/alpha/package.json,packages/zeta/package.json") {
                    throw new Error(`unexpected files: ${files.join(",")}`);
                }
                const content = await host.readText(files[0]);
                if (content !== "alpha") {
                    throw new Error(`unexpected content: ${content}`);
                }
                return {
                    "schema-version": request["schema-version"],
                    diagnostics: [],
                    status: "success",
                    output: {
                        operation: request.operation,
                        output: { packages: [] }
                    }
                };
            }"#,
        );

        runtime.execute(&source, &request(), &plugin_id()).unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "list:packages/**/package.json".to_owned(),
                "read:packages/alpha/package.json".to_owned()
            ]
        );
    }

    #[test]
    fn file_access_is_denied_without_an_injected_backend() {
        let source = module_source(
            r#"async function(_request, host) {
                await host.listFiles("packages/**/package.json");
            }"#,
        );

        assert!(matches!(
            BoaPluginRuntime::default().execute(&source, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("file access is not configured")
        ));
    }

    #[test]
    fn validates_declared_read_patterns_while_loading_metadata() {
        let source = module_source("function() {}").replace(
            "operations: [\"discover\", \"inspect\", \"plan-edits\"]",
            "operations: [\"discover\", \"inspect\", \"plan-edits\"],\n                \"read-patterns\": [\"../secret.txt\"]",
        );

        assert!(matches!(
            BoaPluginRuntime::default().metadata(&source),
            Err(PluginRuntimeError::FileCapability(
                PluginFileError::InvalidPattern { .. }
            ))
        ));
    }

    #[test]
    fn enforces_cumulative_file_capability_budgets_per_operation() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = BoaPluginRuntime::default()
            .with_file_client(RecordingFileClient {
                paths: vec!["alpha.json".to_owned(), "zeta.json".to_owned()],
                content: "123456".to_owned(),
                calls,
            })
            .with_file_limits(8, 10, 3);
        let path_source = module_source(
            r#"async function(_request, host) {
                await host.listFiles("*.json");
                await host.listFiles("*.json");
            }"#,
        );
        let byte_source = module_source(
            r#"async function(_request, host) {
                await host.readText("alpha.json");
                await host.readText("zeta.json");
            }"#,
        );

        assert!(matches!(
            runtime.execute(&path_source, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("returned 4 paths") && message.contains("maximum is 3")
        ));
        assert!(matches!(
            runtime.execute(&byte_source, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("returned 12 bytes") && message.contains("maximum is 10")
        ));
    }

    #[test]
    fn metadata_loading_never_uses_the_configured_network_backend() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime = BoaPluginRuntime::new(RecordingHttpClient {
            requests: requests.clone(),
        });
        let source = r#"
            const response = await fetch("https://api.example.test/metadata");
            export const metadata = await response.json();
            export default function() {};
        "#;

        assert!(matches!(
            runtime.metadata(source),
            Err(PluginRuntimeError::ModuleEvaluation(message))
                if message.contains("network access is not configured")
        ));
        assert!(requests.lock().unwrap().is_empty());
    }

    #[test]
    fn routes_fetch_through_the_injected_host_backend() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime = BoaPluginRuntime::new(RecordingHttpClient {
            requests: requests.clone(),
        });
        let source = module_source(
            r#"async function() {
                const response = await fetch("https://api.example.test/plugin");
                return await response.json();
            }"#,
        );

        runtime.execute(&source, &request(), &plugin_id()).unwrap();
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, "https://api.example.test/plugin");
    }

    #[test]
    fn routes_fetch_through_scoped_origins_and_denies_unlisted_targets() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let runtime = BoaPluginRuntime::default().with_http_transport(
            [PluginHttpOrigin::parse("https://api.example.test").unwrap()],
            RecordingHttpTransport {
                requests: requests.clone(),
            },
        );
        let allowed_source = module_source(
            r#"async function() {
                const response = await fetch("https://api.example.test/plugin");
                return await response.json();
            }"#,
        );
        let blocked_source = module_source(
            r#"async function() {
                await fetch("https://blocked.example.test/plugin");
            }"#,
        );

        runtime
            .execute(&allowed_source, &request(), &plugin_id())
            .unwrap();
        assert!(matches!(
            runtime.execute(&blocked_source, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("origin is not allowed")
        ));
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, "https://api.example.test/plugin");
    }

    #[test]
    fn applies_fresh_http_request_budgets_to_each_operation() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let limits = PluginHttpLimits {
            max_operation_requests: 1,
            ..PluginHttpLimits::default()
        };
        let runtime = BoaPluginRuntime::new(RecordingHttpClient {
            requests: requests.clone(),
        })
        .with_http_limits(limits);
        let one_fetch = module_source(
            r#"async function() {
                const response = await fetch("https://api.example.test/one");
                return await response.json();
            }"#,
        );
        let two_fetches = module_source(
            r#"async function() {
                await fetch("https://api.example.test/one");
                await fetch("https://api.example.test/two");
            }"#,
        );

        runtime
            .execute(&one_fetch, &request(), &plugin_id())
            .unwrap();
        runtime
            .execute(&one_fetch, &request(), &plugin_id())
            .unwrap();
        assert!(matches!(
            runtime.execute(&two_fetches, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("attempted 2 requests") && message.contains("maximum is 1")
        ));
    }

    #[test]
    fn network_is_denied_without_an_injected_backend() {
        let source = module_source(
            r#"async function() {
                await fetch("https://api.example.test/plugin");
            }"#,
        );
        assert!(matches!(
            BoaPluginRuntime::default().execute(&source, &request(), &plugin_id()),
            Err(PluginRuntimeError::EntrypointInvocation(message))
                if message.contains("network access is not configured")
        ));
    }

    #[test]
    fn runtime_handle_is_send_and_sync() {
        fn assert_send_and_sync<T: Send + Sync>() {}
        assert_send_and_sync::<BoaPluginRuntime>();
    }
}
