# Semifold Rust 架构重设计方案

## 1. 文档状态

- 状态：Draft
- 范围：Semifold Rust workspace 的领域模型、crate 边界、生态适配器、版本规划和发布执行流程
- 不包含：文档站、官网视觉、品牌和营销内容；这些内容由
  [文档体验重构方案](documentation-redesign.md) 独立治理
- 原则：优先替换内部模型；CLI 主要用法保持稳定，配置字段在本轮显式统一为 kebab-case

### 1.1 文档与变更治理

本 PRD 是 Semifold Rust 架构的技术事实来源。任何需求、架构或设计变更在实现前必须先更新本 PRD，并在 PRD 与目标实现一致后才可继续。

根目录 `TODO.md` 仅记录本 PRD 与当前代码之间尚未完成的差异：

- 当 PRD 新增了尚未在代码中实现的行为时，必须在 `TODO.md` 新增或更新对应待办；
- 当代码完成某项 PRD 定义的行为并通过适当验证时，必须更新 `TODO.md`，移除或勾选对应待办；
- `TODO.md` 不是独立需求来源；需求以 PRD 为准。

`Cargo.toml`、workspace members、依赖和 crate 元数据不得被手工编辑。所有此类变更必须经由 Cargo CLI 完成，例如 `cargo add`、`cargo remove`、`cargo new` 或 `cargo init`。如果 Cargo CLI 无法表达所需变更、命令失败，或其结果与 PRD 不一致，必须停止并向用户确认，不得回退为手动编辑 `Cargo.toml`。

## 2. 背景

Semifold 的核心定位是管理跨语言 monorepo 中的 changeset、版本、内部依赖、changelog 和包发布。现有实现已经支持 Rust、Node.js、Python 和 C++，但核心设计仍然以单个 `Resolver` 和 CLI 命令流程为中心，没有建立统一的跨生态工作区模型。

这导致不同命令重复计算发布状态，依赖顺序由各 resolver 局部处理，版本修改依赖运行期可变状态，并且领域计算与文件、Git、HTTP 和子进程副作用直接耦合。

## 3. 现状诊断

### 3.1 `Context` 承担过多职责

当前 `Context` 同时保存：

- Semifold 配置；
- changeset 路径与配置路径；
- 仓库根目录；
- GitHub 仓库信息；
- `git2::Repository`；
- resolver 工厂；
- dry-run 状态；
- 版本计算过程中的 `version_bumps` 可变缓存。

`Context::create()` 中多次使用 `.ok()` 将发现或打开错误转换为 `None`。配置不存在、Git 仓库无法打开和路径解析失败因此很难区分。

`RefCell<HashMap<String, Version>>` 使版本修改依赖包处理顺序。Resolver 只能看到此前已经写入缓存的新版本，无法在修改文件前检查完整结果。

当前 `VersionMode::PreRelease` 还将发布通道直接绑定到 SemVer prerelease 字符串：它会在没有 changeset 时推进序号，并以 resolver 的 SemVer 表达作为所有生态的共同规则。这个行为不应成为新设计的兼容目标；新的 `ReleaseChannel` 必须以 changeset 为驱动，并由各生态 adapter 负责版本编码与校验。

### 3.2 `Resolver` 的职责边界错误

当前 `Resolver` trait 同时负责：

1. 发现包；
2. 解析 manifest；
3. 修改 manifest 和版本文件；
4. 排序所有包；
5. 执行发布命令。

其中发布命令来自用户配置，并不是生态解析器的能力。Rust、Node.js、Python 和 C++ resolver 的 `publish()` 实现因此几乎完全重复。

Resolver 还直接依赖 `Context`，导致其无法作为纯粹的生态适配器独立测试。

### 3.3 依赖顺序不是真正的拓扑排序

现有 Rust、Node.js 和 Python resolver 使用 `Vec::sort_by()` 比较两个包是否直接依赖。该 comparator 不具备完整依赖图信息，不能可靠处理传递依赖，也可能不满足排序比较器要求的传递性。

例如：

```text
A → B → C
```

两两比较不能稳定保证得到 `C, B, A`。

每个 resolver 还只处理相同生态的依赖。项目中不存在一张统一的跨语言依赖图，因此无法系统性处理 Rust 包与 Node.js 绑定包等跨生态关系。

### 3.4 `status`、`version` 和 `publish` 重复推导状态

三个命令分别执行包解析、bump 计算和依赖处理：

- `status` 独立计算当前版本与下一版本；
- `version` 再次计算，并立即修改文件；
- `publish` 重新发现和排序包。

这三条路径没有共享同一个发布计划，因此难以保证预览、版本修改和发布顺序始终一致。

### 3.5 领域计算与副作用混合

当前 `version` 流程在单个循环内交错执行：

```text
解析包
→ 计算新版本
→ 写入 manifest
→ 更新 Context.version_bumps
→ 查询 Git/GitHub 元数据
→ 生成并写入 changelog
→ 处理下一个包
```

任一中间步骤失败都可能留下部分修改的工作区。`dry_run` 判断也因此散落在 CLI 和各 resolver 内部。

### 3.6 测试基础不足

当前 Rust 源码中基本没有针对以下核心行为的自动化测试：

- changeset 解析和合并；
- 版本通道与版本序号规则；
- 依赖传播和拓扑顺序；
- 各生态 manifest 的解析与重写；
- dry-run 与真实执行的一致性；
- 发布前检查和部分失败。

在缺少特征测试的情况下直接替换 resolver 实现，风险过高。

## 4. 设计目标

### 4.1 主要目标

1. 建立统一的跨生态 `WorkspaceGraph`。
2. 使 `status` 和 `version` 消费同一个不可变 `ReleasePlan`。
3. 将计算与文件、Git、HTTP、GitHub 和命令执行副作用分离。
4. 使生态适配器可以通过 fixture 独立测试。
5. 使 dry-run 成为执行器策略，而非遍布代码的条件分支。
6. 为依赖环、重复包名、manifest 不一致和文件修改冲突提供明确错误。
7. 保持现有 CLI 主要用法稳定；配置字段统一为 kebab-case，不兼容旧 snake_case 字段。

### 4.2 非目标

- 不在第一阶段重新设计 CLI。
- Semifold 配置只保留 TOML 容器格式；字段命名统一为 kebab-case。
- 不为每个小模块创建独立 crate。
- 不在第一阶段引入动态插件或 WASM resolver。
- 不尝试回滚已经完成的外部 registry 发布。

## 5. 核心设计：Plan → Validate → Apply

Semifold 应当在任何写文件或执行外部命令前，先构建完整、不可变、可序列化的计划。

```text
Config + Changesets + Ecosystem manifests
                    │
                    ▼
              WorkspaceSnapshot
                    │
                    ▼
               WorkspaceGraph
                    │
                    ▼
                ReleasePlan
                    │
          ┌─────────┼─────────┐
          ▼         ▼         ▼
       render    validate    apply
      (status)              (version)
```

`publish` 基于当前 `WorkspaceSnapshot` 和配置构建 `PublishPlan`，然后进行 registry preflight 与顺序执行。

## 6. 领域模型

### 6.1 基础类型

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageId(String);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Ecosystem {
    Rust,
    Node,
    Python,
    Cpp,
}

pub struct PackageSnapshot {
    pub id: PackageId,
    pub manifest_name: String,
    pub version: semver::Version,
    pub ecosystem: Ecosystem,
    pub path: Utf8PathBuf,
    pub publishable: bool,
    pub dependencies: Vec<Dependency>,
}

pub struct Dependency {
    pub package: PackageId,
    pub kind: DependencyKind,
    pub requirement: Option<String>,
    pub source: DependencySource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyKind {
    Unspecified,
    Runtime,
    Development,
    Build,
    Optional,
    Peer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencySource {
    Manifest,
    Config,
}
```

`PackageId` 是 Semifold 配置和依赖图中的稳定身份，不应继续在所有层使用无约束 `String`。加载时应验证配置键、manifest 包名和依赖引用的关系。

`PackageId` 在一份 Semifold 配置中全局唯一，但不自动采用 `ecosystem:name` namespace。manifest name
只要求在同一生态内唯一：不同生态可以声明相同 manifest name，并通过用户选择的稳定 `PackageId`
（例如 `rust-shared` 与 `node-shared`）区分；同一生态内出现重复 manifest name 时无法无歧义绑定
manifest 依赖，工作区加载必须失败。manifest 依赖始终按 `(Ecosystem, manifest_name)` 解析，绝不因
跨生态名称相同而推断跨生态边；跨生态关系只能通过 `depends-on` 引用稳定 `PackageId`。

首次 `init` 从 manifest name 生成初始配置时，只对跨生态重复建议自动加 ecosystem 前缀，例如
`shared` 分配为 `rust-shared` 与 `nodejs-shared`；未冲突的建议保持原样。若限定后的 ID 已被其他建议
占用，按完整 discovery 稳定顺序追加 `-2`、`-3`，直到得到全局唯一 ID。同一生态内的重复 manifest
name 仍然失败，不能用自动后缀掩盖依赖匹配歧义。该规则只负责生成首次配置，不改变已有稳定 ID，也
不让运行时从 ID 反向推导 ecosystem。

`DependencySource` 区分生态 manifest 推导的依赖与 `depends-on` 配置补充的依赖。两者都参与同一个
`WorkspaceGraph`；同一 package 同时通过两种来源指向同一目标时，图边去重，但配置来源的发布传播语义仍必须保留。

ecosystem adapter 返回的 `publishable` 表示从原生 manifest 推导出的缺省发布资格，例如 Rust
`publish = false` 与 Node.js `private = true`。`PackageConfig` 增加可选布尔字段 `publish`：缺省时不写入
配置，并沿用 adapter 结果；显式 `true` 或 `false` 时覆盖 adapter 结果，得到进入
`PackageSnapshot.publishable` 的有效值。该覆盖只控制 Semifold 是否执行 registry preflight 与配置的
发布命令，不修改原生 manifest；强制设为 `true` 时，用户仍需保证原生发布工具和自定义命令接受当前
manifest。`config sync` 必须保留已有覆盖，新发现的软件包保持 `publish = None`。

### 6.2 `WorkspaceGraph`

```rust
pub struct WorkspaceGraph {
    packages: BTreeMap<PackageId, PackageSnapshot>,
    dependencies: DependencyGraph,
}
```

`WorkspaceGraph` 负责：

- 检测重复 `PackageId`；
- 解析内部依赖边；
- 合并 manifest 依赖和配置补充的跨生态依赖；
- 检测依赖环并返回完整环路；
- 生成确定性拓扑顺序；
- 为版本传播和发布顺序提供统一输入。

拓扑排序由 core 一次完成，不再由各生态 adapter 修改同一个 `Vec`。对于无依赖关系的节点，使用 `PackageId` 排序保证输出稳定。

首个切片以 `WorkspaceGraph::new(Vec<PackageSnapshot>)` 接收已发现的 package。构建时仅将指向图内 package 的依赖作为内部依赖边；引用未知 package 时返回包含依赖方与被引用 `PackageId` 的领域错误。重复 `PackageId` 同样返回领域错误。`topological_order()` 返回“依赖在前、依赖方在后”的稳定顺序；若存在环，错误携带首尾相连的完整 `PackageId` 环路，便于后续 CLI 渲染诊断。

### 6.3 `ReleasePlan`

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChangesetId(String);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BumpLevel {
    Unchanged,
    Patch,
    Minor,
    Major,
}

pub type VersionMap = BTreeMap<PackageId, semver::Version>;

pub struct ReleasePlan {
    packages: Vec<PackageRelease>,
    versions: VersionMap,
    order: Vec<PackageId>,
    consumed_changesets: Vec<ChangesetId>,
    warnings: Vec<PlanWarning>,
    file_edits: Vec<FileEdit>,
}

pub struct PackageRelease {
    pub id: PackageId,
    pub ecosystem: Ecosystem,
    pub current_version: semver::Version,
    pub next_version: semver::Version,
    pub bump: BumpLevel,
    pub reasons: Vec<ReleaseReason>,
}

pub enum ReleaseReason {
    Changeset { changeset: ChangesetId },
    DependencyPropagation {
        dependency: PackageId,
        next_version: semver::Version,
    },
}

pub enum PlanWarning {
    NonPatchBumpOnPrerelease {
        package: PackageId,
        requested: BumpLevel,
    },
}
```

`ReleasePlan.packages` 只包含本次实际发布的 package，`versions` 则必须包含工作区所有 package 的计划后版本，未发布 package 使用当前版本。生态 adapter 因此可以一次获得完整 `VersionMap`，不再需要 `Context.version_bumps`。

`ReleasePlan` 只通过验证构造函数创建并提供只读访问器：每个发布 package 必须在 `versions` 中存在且版本等于 `next_version`；`order` 必须无重复地包含所有且仅包含发布 package；`consumed_changesets` 不得重复。违反这些条件返回领域错误。集合顺序由 planner 按 `PackageId`、`ChangesetId` 和 `WorkspaceGraph` 拓扑顺序确定，使序列化结果稳定。同一 changeset 或依赖传播原因在一个 package 上不得重复记录。

纯 planner 接收已解析的领域输入，不读取 changeset 文件、config 或 manifest：

```rust
pub struct ChangesetInput {
    pub id: ChangesetId,
    pub releases: BTreeMap<PackageId, BumpLevel>,
}

pub struct PackageReleasePolicy {
    pub channel: ReleaseChannel,
    pub propagating_dependencies:
        BTreeMap<PackageId, Option<semver::VersionReq>>,
}

pub struct ReleasePlanner;

impl ReleasePlanner {
    pub fn plan(
        graph: &WorkspaceGraph,
        changesets: &[ChangesetInput],
        policies: &BTreeMap<PackageId, PackageReleasePolicy>,
    ) -> Result<ReleasePlan, ReleasePlannerError>;
}
```

`propagating_dependencies` 是 adapter/engine 根据生态规则筛选并解析后的传播策略，而不是 manifest
所有依赖的副本。依赖是否参与排序与是否触发发布传播是两项独立决策：

- 所有已解析为内部边的 manifest `Runtime`、`Development`、`Build`、`Optional`、`Peer` 依赖都参与
  `WorkspaceGraph` 拓扑排序，确保版本修改、构建、测试与发布命令始终在依赖之后执行；
- 首版只有 Rust `[dependencies]` 的 `Runtime` 边进入约束感知的自动传播；
- manifest `Development`、`Build`、`Optional` 与 `Peer` 边不自动传播；Node.js、Python 与 C++ 的
  manifest `Runtime` 边也不自动传播，因为 npm、PEP 440 与 CMake 约束不能交给 Rust semver
  解析器近似判断；
- `source = Config` 的 `depends-on` 边不受类别限制，始终以无约束策略传播 patch，作为需要重新构建
  或重新发布时的显式选择。

`Some(requirement)` 在依赖的新版本仍满足约束时不传播；`None` 表示没有可验证的发布约束，依赖发生
发布时传播 patch。未来若为其他生态启用 manifest 自动传播，必须先由对应 adapter 将原生约束转换为
可验证的领域策略，不得在 application/core 中用 `semver::VersionReq` 猜测 npm 或 PEP 440 语义。

planner 合并同一 package 的最高 bump，并为每个贡献 changeset 保留独立原因。依赖约束失效时，将尚未发布的依赖方加入 patch 发布闭包；依赖方已有显式发布时保留其更高 bump，同时追加依赖传播原因。完整闭包计算后一次生成所有 package 的 `VersionMap`，再按 `WorkspaceGraph` 拓扑顺序生成发布顺序。

changeset 只能在 manifest、changelog 和 post-version 命令均成功后删除。post-version 失败时保留已经写入的文件和全部 changeset，不尝试跨进程回滚；`ApplyReport` 必须列出已替换文件、失败命令和仍待消费的 changeset，并提供恢复所需的事实。

`status` 只渲染该计划，`version` 验证并应用该计划。

在完整 `PublishPlan` 落地前，现有 `publish` 迁移层也必须使用同一个
`WorkspaceGraph.topological_order()` 生成 package 执行顺序；不得继续依次调用各
resolver 的局部 `sort_packages()`，因为直接依赖比较无法形成多层依赖的全局拓扑序，
会导致依赖包尚未发布时先发布依赖方。完整发布引擎仍在阶段 5 接管 preflight、skip
reason、命令和报告，但迁移期发布顺序必须立即满足“依赖先于依赖方”。

### 6.4 `FileEdit`

```rust
pub struct FileEdit {
    pub path: Utf8PathBuf,
    pub expected_hash: FileHash,
    pub new_content: String,
    pub source: EditSource,
}
```

Adapter 生成内容修改，但不直接写文件。`expected_hash` 用于确保从规划到执行期间目标文件没有被其他进程改动。

`FileHash` 必须是规划时读取到的原始文件字节的 SHA-256 摘要，并以小写十六进制编码。领域层通过 `FileHash::from_bytes(&[u8])` 生成该值；不接受由调用方任意构造的字符串。执行器在写入任何临时文件前读取每个目标文件并比较摘要，任一不匹配即返回冲突且不得写入任何目标文件。

执行前还应检查多个修改是否同时针对同一文件。同一文件的多个变更必须先合并或报告冲突，不允许依赖执行顺序相互覆盖。

第一版由应用层的 `FileEditExecutor` 接收项目根目录与 edit 列表。它只接受项目根目录内的相对路径，拒绝绝对路径和包含 `..` 的路径；先验证全部路径、重复目标和 `expected_hash`，随后把所有新内容写入同目录临时文件，最后逐个 `rename` 替换目标文件。任一前置验证或临时文件写入失败时不得替换任何目标文件，并应尽力清理已创建的临时文件；替换阶段发生 I/O 失败时返回已替换与未替换文件的结构化报告，恢复策略留待后续 `ApplyReport` 统一定义。

只读目标验证必须暴露为独立于 `FileEditExecutor` 的入口，并由 executor 与
`version --dry-run` 复用；dry-run 不得构造临时文件或收集远程 changelog 元数据。
配置中显式声明 `dry-run = true` 的命令是例外：它表示用户授权该命令在全局 dry-run
下仍由 `CommandRunner` 执行；未声明或为 `false` 的命令跳过。验证列表中任一较晚目标失败时，先前目标也不得被替换；临时文件准备
阶段失败时同样不得开始替换，并应清理已准备的临时文件。最终文件内容由已验证且目标唯一
的完整 edit 集合决定，不依赖 edit 或 package 的输入顺序。

`ReleasePlan` 只表达版本计算、发布闭包、顺序和领域文件修改，是 `status` 与 `version` 共享的
纯领域事实。它不得为了执行 `version` 而携带项目根目录、配置、Git 句柄或 dry-run 状态。
应用层在真正执行前将其准备为完整的 `ReleaseApplyPlan`：

```rust
pub struct ReleaseExecutionOptions {
    pub collect_remote_metadata: bool,
    pub repository: Option<RepositoryContext>,
}

pub enum ExecutionMode {
    Apply,
    DryRun,
}

pub struct ReleaseApplyPlan {
    pub release: ReleasePlan,
    pub project_root: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub changelogs: BTreeMap<PackageId, String>,
    pub changesets_to_remove: Vec<Utf8PathBuf>,
    pub channel_bumps_to_consume: Vec<PackageId>,
    pub post_version_commands: Vec<PostVersionCommand>,
    pub remote_metadata_failures: Vec<PackageId>,
}

pub struct PostVersionCommand {
    pub package: PackageId,
    pub command: CommandSpec,
}
```

`prepare_release(project, release, options)` 负责加载并核对待消费 changeset、收集允许的 Git/Forge
元数据、生成 changelog edit、规划 post-version 命令，并在产生副作用前验证完整计划。dry-run 调用方
必须将 `collect_remote_metadata` 设为 `false`，保持 dry-run 不访问远程 changelog 元数据的既有约定。
`apply_release(plan, mode)` 只消费已经准备好的应用计划：`DryRun` 不写 Semifold 文件、不删除
changeset、不消费 `channel-bump`，但仍执行 `run_in_dry_run = true` 的 post-version 命令；`Apply`
在文件修改和全部 post-version 命令成功后才删除 changeset 并消费一次性 channel bump。

该两层计划避免把执行环境污染到 core `ReleasePlan`，也避免 application service 重新依赖进程全局
状态或旧的万能 `Context`。

### 6.5 `PublishPlan`

```rust
pub struct PublishPlan {
    pub project_root: Utf8PathBuf,
    pub packages: Vec<PackagePublish>,
}

pub struct PublishOptions {
    pub create_forge_release: bool,
    pub repository: Option<RepositoryContext>,
}

pub struct PackagePublish {
    pub context: PublishContext,
    pub preflight: Option<RegistryCheck>,
    pub commands: Vec<CommandSpec>,
    pub assets: Vec<AssetDeclaration>,
    pub forge: Option<PackageForgePlan>,
    pub skip_reason: Option<PublishSkipReason>,
}

pub struct PublishContext {
    pub package: PublishPackageContext,
    pub repository: Option<RepositoryContext>,
    pub ci: Option<CiContext>,
}

pub struct PublishPackageContext {
    pub id: PackageId,
    pub name: String,
    pub ecosystem: Ecosystem,
    pub version: semver::Version,
    pub tag: String,
    pub path: Utf8PathBuf,
    pub private: bool,
}
```

Registry pre-check、发布命令和 GitHub release 不应存在于 ecosystem adapter 中。它们由 engine 根据统一包模型和用户配置组装。

pre-check 是 publish 前对“目标 package version 是否已经存在”的只读探测，不承担首次发布历史
查询。配置使用带 `type` 判别字段的强类型结构：

```toml
[resolver.rust.pre-check]
type = "http"
url = "https://crates.io/api/v1/crates/{{ package.name }}/{{ package.version }}"
retry = [2, 5, 15, 30]
```

HTTP pre-check 只有 `200 OK` 表示目标版本已存在，`404 Not Found` 表示不存在；鉴权、限流、服务端
错误及其他状态均视为 preflight 失败，不得推断为可发布。HTTP runner 必须在运行时注入包含当前
engine 版本与项目地址的默认 `User-Agent`；配置在 `extra-headers` 中显式提供的 `User-Agent`
大小写不敏感地覆盖默认值，`init` 不得将运行时默认值固化到配置。

HTTP pre-check 可配置以秒为单位的 `retry` 延迟数组。首次请求立即执行；传输错误以及 `408`、
`425`、`429`、`500`、`502`、`503`、`504` 按数组顺序等待并重试，其他状态不重试。响应提供有效
`Retry-After` 时优先使用该值；配置缺省为空数组且不重试，`init` 对 HTTP resolver 显式写入
`[2, 5, 15, 30]`。计划必须保留已渲染 URL、headers 与 retry 延迟，执行器不得重新读取配置。

非 `200`/`404` 的最终响应错误必须包含 status、有效的 `Retry-After`、常见 request ID 响应头以及
最多 4 KiB 的 UTF-8 lossy 响应正文；正文超限时明确标记截断。错误不得输出请求 headers，避免把
registry 凭据写入日志。读取错误响应正文失败时仍返回原 HTTP status 与可用响应头。

需要自定义 registry 或本地策略时可配置 command pre-check：

```toml
[resolver.rust.pre-check]
type = "command"
command = "./scripts/version-exists"
args = []
```

command pre-check 在 package 工作目录中运行，stdin 接收单行 JSON `PublishPackageContext`，其后附加
换行；stdout 必须只返回单行 JSON `{"exists": true}` 或 `{"exists": false}`。进程退出码必须为
0；非零退出、无法启动、无效 JSON、额外非空 stdout 内容均为 preflight 失败，stderr 继承以保留
诊断信息。pre-check 在普通 publish 和全局 dry-run 中都执行，必须由用户保证只读。它不复用发布
命令的 `stdout`、`stderr` 或 `dry-run` 配置，因为协议要求 stdin/stdout 固定为 pipe。

`PublishPlan` 在 publish 进程中根据当前 `WorkspaceSnapshot`、强类型配置、显式
`PublishOptions` 和
`WorkspaceGraph` 重新构造，不依赖已被 version 消费的 changeset，也不持久化或恢复
`ReleaseContext`。`PublishContext` 是单个 package 的只读模板快照；pre-check、
prepublish、publish、asset 和 package GitHub Release 只消费 `package.*` 以及可选的
repository/CI 事实。首版继续按确定性拓扑顺序检查当前配置中的 package；registry 中已存在的
版本和缺失 changelog 通过显式 `skip_reason` 表示，而不是借助历史 `ReleasePlan` 推断本次发布
集合。private package 只跳过 registry preflight 与发布命令，不作为 package 级 skip reason；
它是否创建 GitHub Release 由 package 发布策略独立决定。

这里的 public/private 判断使用 package 的有效 `publishable`：`packages.<PackageId>.publish` 显式值
优先于 ecosystem adapter 从 manifest 推导的缺省值。`publish = false` 因而可以让 Python、C++、插件
生态或任意内部工具跳过 registry 流程；`publish = true` 则允许 Semifold 对原生 manifest 标记为
private 的 package 执行配置的 preflight 与发布命令，但不负责改写或绕过原生工具自身的发布限制。

`PackageConfig` 新增可选的 kebab-case 字段 `github-release`。该字段缺省时保持兼容策略：
有效 publishable package 默认创建 GitHub Release，private package 默认不创建；显式 `true` 允许任意
package 创建，显式 `false` 禁止任意 package 创建。最终行为同时受运行入口的全局开关约束：
`PublishOptions.create_forge_release = false` 始终禁止创建，不能被 package 配置覆盖。

`create_forge_release = true` 时必须同时提供 `RepositoryContext`；engine 对全局开关与 package
`github-release` 策略都允许的 package，在规划阶段读取并验证最新 changelog，将完整
`PackageForgePlan` 固化到对应 `PackagePublish`。不创建 Forge release 时不读取 changelog 正文，
但所有 package 仍必须通过 changelog 存在性检查。项目根目录作为不可变执行事实保存在
application 层 `PublishPlan`，只用于命令工作目录和延迟 asset 解析，不进入 package 模板 context。

所有 package 都必须存在 `<package.path>/CHANGELOG.md` 才能进入 registry preflight、命令或
Forge release 流程。缺失时以 `PublishSkipReason::MissingChangelog` 显式跳过，优先级高于 private
registry skip。存在但无法解析的 changelog 是计划错误，不允许先发布 package 后才发现无法创建
release。

publisher 在执行任何 package 命令前先完成所有非 private、未跳过 package 的 registry
preflight；private package 不执行 registry preflight 或 package 发布命令，但如果其有效
`github-release` 策略为 true，仍继续创建 GitHub Release 并上传 asset。preflight 失败时不启动
任何命令；版本已存在则以 `PublishSkipReason::RegistryVersionExists` 跳过该 package 的 registry
命令，但不能跳过已经规划的 GitHub Release：Release 尚不存在时仍创建并上传本次 CI 中可用的
asset，Release 已存在时报告 `ForgeDisposition::AlreadyExists` 且不补传、覆盖或核对 asset。随后按
`PublishPlan.packages` 的拓扑顺序逐包执行命令、创建 package release 并上传 asset，任一阶段失败即
停止后续 package。GitHub Release 创建后发生 asset 上传失败时不提供续传恢复；用户应修复构建或
CI/CD 问题并发布新版本。`PublishPlan`
只保存已经过语法和路径校验的 `AssetDeclaration`，不得在命令执行前展开 glob 或过滤不存在文件；
因为 asset 可以由 prepublish/publish 命令生成。package 命令成功后，执行器才通过注入的
`AssetResolver` 展开声明，生成稳定排序的 `ReleaseAsset`，再由 `FileSystem` 读取并交给
`ForgeClient` 上传。缺失或无效的显式 asset、glob 未匹配到预期产物以及读取失败必须进入该
package 的结构化失败报告，不得静默省略。

执行结果始终表示为结构化报告：

```rust
pub struct PublishReport {
    pub packages: Vec<PackagePublishReport>,
}

pub struct PackagePublishReport {
    pub package: PackageId,
    pub status: PublishStatus,
    pub commands: Vec<CommandReport>,
    pub forge: ForgeDisposition,
    pub error: Option<String>,
}

pub enum PublishStatus {
    Succeeded,
    Skipped(PublishSkipReason),
    Failed(PublishFailureStage),
    NotStarted,
}
```

命令、registry 与 Forge 错误通过 `PublishExecutionError` 携带当时的完整报告返回；CLI 返回非零
退出码，并提示修复后重试。重试依赖 registry preflight 跳过已成功发布的版本，不尝试回滚外部
registry；其中 registry version skip 与 Forge disposition 是两个独立事实，CLI 必须在同一 package
行中同时展示，不能以笼统的“跳过”隐藏已经创建或已经存在的 GitHub Release。dry-run 仍执行全部
registry preflight；命令报告区分实际执行与因未配置
`dry-run = true` 而跳过，Forge disposition 明确为 dry-run skip。

在完整 `PublishPlan` 落地前，阶段 4 使用 application 层的统一发布命令执行桥接：按 package 的
拓扑顺序依次执行全部 `prepublish`，成功后再执行全部 `publish`；命令工作目录为 package path，任一
命令失败立即停止该 package 和后续发布。全局 dry-run 时，仅执行配置中
`dry-run = true` 的命令，其余命令明确报告跳过。private package、registry pre-check、GitHub Release
与 assets 仍由同一 application 流程编排，不回到 adapter。四个旧 resolver 不再暴露 `publish()`，
也不接收 dry-run；该桥接后续由 `PublishPlan` 与注入的 `CommandRunner` 取代。

### 6.6 Workspace 级 `ReleaseContext` 与分层模板视图

一次 Semifold release 的技术事实是完整 workspace `ReleasePlan`，而不是某个隐式主 package
的版本或 tag。workspace release 天然是 `PackageId -> next_version` 的集合；多包计划不
自动具有单一 `release.version`、`release.tag` 或一个需要额外配置的“release identity”。
因此当前设计不引入 `ReleaseUnit`、`ReleaseIdentityStrategy` 或
`ResolvedReleaseIdentity`。version 阶段的分支与 release PR 消费一个 workspace 级
`ReleaseContext`；它不跨 release PR 持久化，也不是 publish 阶段的输入。registry、发布命令、
package tag 和 package GitHub Release 使用从当前 workspace 重建的 `PublishContext`。

`ReleaseContext` 是在 `ReleasePlan` 完成后构造的不可变、可序列化事实快照：

```rust
pub struct ReleaseContext {
    pub plan: ReleasePlanContext,
    pub repository: Option<RepositoryContext>,
    pub ci: Option<CiContext>,
}

pub struct ReleasePlanContext {
    pub packages: BTreeMap<PackageId, PackageReleaseContext>,
    pub changesets: Vec<ChangesetId>,
    pub common_version: Option<semver::Version>,
    pub fingerprint: String,
}

pub struct PackageReleaseContext {
    pub id: PackageId,
    pub ecosystem: Ecosystem,
    pub current_version: semver::Version,
    pub next_version: semver::Version,
    pub bump: BumpLevel,
    pub reasons: Vec<ReleaseReasonContext>,
}

pub struct RepositoryContext {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub web_url: String,
    pub commit: Option<CommitContext>,
}

pub struct CiContext {
    pub provider: CiProvider,
    pub run_id: Option<String>,
    pub run_url: Option<String>,
    pub ref_name: Option<String>,
}
```

`ReleasePlanContext.packages` 只包含本次实际发布的 package，使用 `BTreeMap`保证
序列化稳定。`common_version` 是该集合的派生属性：仅当集合非空且所有
`next_version` 相同时存在；它不触发 lockstep bump，也不使 workspace 获得隐式
版本。`fingerprint` 是计划事实，不是 release identity：它以按 `PackageId` 排序的
`(PackageId, next_version)` 和按 ID 排序的已消费 changeset 为规范化输入，计算
SHA-256 并输出前 12 位小写十六进制。它不包含路径、遍历顺序、时间、
Git SHA、远程元数据或 changeset 文本。

`RepositoryContext` 和 `CiContext` 只保存收集完成的可序列化事实，不持有
`git2::Repository`、Forge client、token、完整环境变量或命令执行器。非必需远程
元数据收集失败时保留诊断并使用 `None`，不得回滚或重新计算 `ReleasePlan`。

release PR 在 version 文件 edit 与 changelog 内容完成规划后，由 application 层构造
一次性的只读视图；changelog 不写回 `ReleaseContext`，也不扩张 workspace release
事实模型：

```rust
pub struct ReleasePullRequestContext<'a> {
    pub release: &'a ReleaseContext,
    pub branch: String,
    pub changelogs: BTreeMap<PackageId, String>,
    pub oversized_body_notice: &'a str,
}

pub struct RenderedReleasePullRequest {
    pub title: String,
    pub body: String,
}
```

release commit message 与 release PR title 可以通过仓库级可选配置定制：

```toml
[release]
commit-message = "chore(release): {{ release.plan.fingerprint }}"
pull-request-title = "chore(release): {{ release.plan.common_version }}"
```

两个字段都只接收严格 MiniJinja 模板，并只暴露与 `branches.release` 相同的 `release.*`
作用域；不得把项目路径、原始配置、changelog、token 或任意环境变量加入模板上下文。
模板在 version 文件副作用发生前完成渲染与校验：commit message 渲染结果必须非空，PR title
还必须是非空单行文本。字段或整个 `[release]` 分区缺省时分别使用兼容默认值
`chore(release): bump versions`；`init`、`config sync` 与其他配置保存路径不得为了表达默认值
自动写入该分区。

PR 正文不开放用户模板，继续由纯 renderer 以 `# Releases` 开始，并按 `PackageId` 稳定排序
追加各 package changelog。`ReleasePullRequestContext.branch` 是已经由同一个 `ReleaseContext`
渲染并校验的 release branch，供后续 Forge 边界创建或更新 PR；renderer 不从 package 集合中
推断主 package，也不将 changelog 塞回 `ReleaseContext` 或恢复全局万能模板 map。

发布 PR 创建与更新共用同一正文长度保护：以 65,536 UTF-8 字节作为保守预算，确保不超过
GitHub 的 65,536 字符限制。预算内保持原正文逐字不变；超限则保留 `# Releases`，改为按
`PackageId` 稳定排序的当前版本 → 下一版本摘要，不修改完整 changelog 文件。应用层通过
`rust-i18n` 提供 `oversized_body_notice`，说明完整变更日志与包列表位于 PR 的 Files changed，
正文摘要受长度限制。摘要只追加预算内的完整 package 行；单行或包集合过大时停止追加，
不截断 Markdown 行或 Unicode 字符。提示本身异常超长时在 UTF-8 边界截断，仍保证预算。
renderer 不访问网络、环境变量或文件系统，不扩张 `ReleaseContext`，也不新增配置项。
测试覆盖原格式兼容、预算边界、中文与 emoji、多包排序、摘要超限和异常长提示；创建与更新
必须继续消费同一个 `RenderedReleasePullRequest.body`。

当前没有已证明的项目级模板字段，因此首版不引入 `ProjectContext`。应用层的
`Project` 仍负责 root、changeset directory、config path 和强类型配置的加载；这些
运行时路径不默认暴露给模板。只有在出现明确消费者和配置来源后，才能增加
最小、只读的 `ProjectContext`。

### 6.7 GitHub Actions 工作流输出

`smif version` 与 `smif publish` 在 GitHub Actions 中向后续 step/job 暴露版本化 JSON
事实。首版不增加 CLI flag：仅当 `GITHUB_ACTIONS` 严格等于 `true` 且 `GITHUB_OUTPUT`
存在时自动启用；其他环境不创建或写入任何额外文件，也不改变终端输出。两个命令分别写入
`semifold-version` 与 `semifold-publish` output key。首版 schema version 为整数 `1`；同一
major schema 内只允许增加可选字段，删除、重命名字段或改变既有枚举含义必须提升
`schema-version`，并至少保留前一个 schema 一个 Semifold minor 发布周期。

`smif ci` 是上述两个命令的 GitHub Actions 编排入口，不能绕过它们的 CLI application 路径后直接
调用 engine 并丢失 output。存在 changeset 时必须委托 version 的 prepare/apply/output 路径；没有
changeset、进入发布时必须委托 publish 的 plan/execute/report/output 路径。这样 success、dry-run、
publish 部分失败和 output writer 失败继续使用同一优先级，`ci` 不重新构造 DTO 或复制 JSON 写入。
一次 `ci` 执行只产生实际运行分支对应的 output，另一 output 保持未定义。

内置 GitHub Actions workflow 中执行 `semifold ci` 的 step 使用稳定 ID `semifold`。所在 job 将
`steps.semifold.outputs['semifold-version']` 与 `steps.semifold.outputs['semifold-publish']` 分别映射为
稳定 job output `version` 与 `publish`，使后续 job 可通过 `needs.<job>.outputs.version` 或
`needs.<job>.outputs.publish` 消费；未运行分支对应的 job output 是空字符串。项目仓库中实际使用的
workflow 与内置模板必须遵循同一映射契约。

启用 `smif init` 生成的 GitHub Actions workflow 前，仓库必须在 **Settings → Actions → General →
Workflow permissions** 中选择 **Read and write permissions**，并勾选 **Allow GitHub Actions to
create and approve pull requests**。前者允许发布工作流推送版本提交，后者允许其创建或更新发布
Pull Request；组织或企业策略覆盖仓库设置时，管理员还必须在上级策略中允许这些能力。公开文档必须
在首次发布指南生成 GitHub Actions workflow 后、首次运行 workflow 前同步记录此前置条件；`init`
命令参考只描述命令输入与产物，不承载仓库托管平台的后续配置步骤。

version output 从 `prepare_release` 构造的同一个 `ReleaseContext` 以及由该 context 渲染、校验的
release branch 派生，不得在 changeset 消费后重新规划：

```rust
pub struct VersionWorkflowOutput {
    pub schema_version: u32,
    pub dry_run: bool,
    pub plan_fingerprint: String,
    pub release_branch: String,
    pub packages: BTreeMap<PackageId, VersionWorkflowPackage>,
}

pub struct VersionWorkflowPackage {
    pub current_version: semver::Version,
    pub next_version: semver::Version,
}
```

publish output 从执行所用的 `PublishPlan` 与成功报告或 `PublishExecutionError.report` 派生。package
按计划顺序输出，并保留 `succeeded`、`skipped`、`failed`、`not-started` 状态；skip reason 与
failure stage 使用稳定 kebab-case 字符串。恢复所需事实仅包括 package ID、目标版本、状态、
skip reason 与 failure stage，不输出外部错误文本、命令、参数或环境：

```rust
pub struct PublishWorkflowOutput {
    pub schema_version: u32,
    pub dry_run: bool,
    pub packages: Vec<PublishWorkflowPackage>,
}

pub struct PublishWorkflowPackage {
    pub package: PackageId,
    pub version: semver::Version,
    pub status: String,
    pub skip_reason: Option<String>,
    pub failure_stage: Option<String>,
}
```

engine 提供统一、可序列化的 application workflow output DTO；CLI 最外层的 writer 仅负责
GitHub Actions 文件协议。writer 使用进程内逐条唯一且不出现在 payload 中的 delimiter 写入
`name<<delimiter` 多行格式，并以单次 append 保持一条 output 的完整性。DTO 采用字段 allowlist：
禁止加入 HTTP header、环境变量、token、命令配置、完整配置、绝对路径、commit author email；
新增字段必须经过序列化快照和敏感字段回归测试。

退出优先级如下：

- dry-run 仍输出完整 DTO，并以 `dry-run = true` 明确其没有应用文件或 Forge 副作用；
- version 或 publish 成功后 output 写入失败，命令返回非零；
- publish 部分失败时，先尽力写入携带完整恢复状态的 publish output，再返回原 publish 错误；
  若此时 output 写入也失败，publish 错误保持主错误，writer 错误作为 CLI warning；
- version 在形成可用 apply 结果前失败时不伪造成功 output，保留原失败语义。
- `ci` 在 version apply 成功后、Forge PR 操作前写入 version output；因此后续 PR 网络操作失败时，
  使用 `always()` 的恢复 job 仍可读取已经形成的确定性版本事实。

### 6.8 CLI 终端反馈与渲染边界

CLI 的人类可读输出必须形成统一的 presentation 层，不再由各命令任意混用 `println!`、颜色和
`log::*`。core 与 engine 继续只返回结构化 plan/report；CLI 将其投影为只读 presentation model，
再由 `Terminal` 渲染标题、阶段、步骤、表格、成功摘要、警告、失败状态和恢复建议。i18n、颜色、
Unicode 与动态终端状态只存在于 CLI。

命令返回到进程入口的最终错误同样属于 presentation，而不是日志事件：两个 binary 入口必须复用
同一个 CLI 执行函数，由 `Terminal` 向 stderr 输出本地化错误标签和错误内容，再返回失败退出码；
不得调用 `log::error!` 伪装用户可操作错误。错误包装不能丢失底层诊断；配置加载失败必须保留
`ResolveError` 的完整展示，包括 TOML 行列、源码片段和解析原因，再在其后追加独立的恢复建议。

`indicatif` 仅作为 `Terminal` 内部的动态进度适配器：未知长度操作使用 spinner；只有存在可观察的
逐项执行回调时，已知 package、file edit 或 asset 数量才使用有界进度，不能为瞬时静态渲染伪造
进度；`MultiProgress` 只在确有多个同时活动的任务时使用。静态表格、
最终摘要、错误语义、JSON 和 GitHub Actions output 不由 `indicatif` 决定。CLI 通过最小
`ProgressReporter` 边界表达 begin/succeed/skip/fail；交互终端使用 indicatif 实现，非 TTY、CI、
重定向及测试使用稳定的 plain/recording 实现。

输出流契约如下：

- 正常结果、静态表格和成功摘要写 stdout；
- 动态进度、warning、error、debug 和恢复建议写 stderr；
- 非 TTY 不绘制 spinner、进度条或光标控制序列，改为稳定、逐行的阶段结果；
- 遵守 `NO_COLOR` 与 `TERM=dumb`；无法可靠显示 Unicode 时使用 ASCII 标记；
- dry-run 在命令开头显示醒目标识，并在结尾明确哪些操作未应用；
- 成功命令必须给出最终摘要；部分失败必须同时展示 succeeded、skipped、failed 与 not-started
  事实，并给出可执行恢复建议；
- 动态区域存在时，普通消息必须经 `ProgressBar::suspend` 或统一 progress manager 输出，不能破坏
  光标状态；
- 以 inherited stdout/stderr 执行外部命令时，必须在整个子进程生命周期暂停动态区域；子进程退出后
  才可恢复 spinner 并渲染该阶段的最终状态，不能让定时 tick 覆盖子进程输出；
- post-version 命令继续由 engine 按计划顺序执行，并通过 release-apply callback 报告逐命令的
  started/finished 事件；CLI 在批次开始时明确说明顺序执行，并以紧凑、去重且保持首次出现顺序的
  package 列表说明执行范围，不能在执行前一次性渲染多条“正在运行”。
  当命令 stdout 与 stderr 均不继承终端时，为当前命令展示 spinner；任一流继承终端时不展示动态
  状态，仅在子进程退出后渲染该命令的成功或失败结果；
- 表格与键值事实列必须按 Unicode 显示宽度计算 padding，不能使用 Rust 字符数量格式化宽字符标签；
- 面向交互终端的结果表必须在完成列宽计算后应用一致的语义颜色，避免 ANSI 控制序列破坏对齐；
  package 标识使用强调色，版本使用版本色，succeeded、skipped、failed 与 not-started 分别使用
  成功、提醒、失败与弱化色。非 TTY、`NO_COLOR` 与 `TERM=dumb` 下仍退化为内容完全相同的纯文本；
- 中文 CLI 文案优先使用短句、逗号或句号，避免用分号串联状态、结果、提示或恢复操作；只有保留
  用户输入、外部错误或协议原文时才不改写其中的分号；
- `--debug` 不得打印完整配置、GitHub event、header、环境变量、token、命令环境或其他敏感值。

全局 `--dry-run` 是所有命令共享的副作用边界，而不只是部分 release 命令自行解释的展示参数。
启用后，CLI 仍完成当前命令的参数解析、领域规划、输入校验、模板渲染和必要的只读查询，但不得
创建、更新或删除 Semifold 管理的文件，不得修改 registry 或 Forge 资源，也不得创建或更新
GitHub PR comment。`init` 只生成并报告 `InitPlan`，不调用 `apply_init()`；`commit` 必须完整校验并
渲染候选 changeset，但不创建文件；`status --comment` 可以准备评论展示所需的只读事实，但不得
调用评论写入接口。配置中显式声明 `dry-run = true` 的外部命令是唯一例外，它表示用户授权该命令
在全局 dry-run 中执行；该许可不扩大 Semifold 自身文件、registry、Forge 或 PR comment 的写入范围。
纯只读命令在 dry-run 下保持相同领域结果，但仍需在开头和结尾明确报告未应用副作用。

首个切片改造 `status`、`version` 与 `publish`：status 展示 changeset/package 数量、fingerprint、
版本、bump 和原因；version 展示准备、验证、应用、post-version 与 changeset 消费事实；publish
展示 preflight、命令、Forge、asset 和四态结果。随后统一 config、init、commit 和 CI 的标题与最终
反馈。展示层必须有 TTY/非 TTY、dry-run、成功、skip、部分失败、宽字符和敏感信息回归测试；测试
使用内存 writer 或 indicatif `TermLike`，不得依赖人工观察终端。

`status` 的 GitHub PR comment 是独立 Markdown presentation：有发布计划时直接展示 changeset/package
摘要以及包含当前版本、目标版本、bump 和原因的表格，不将主要信息折叠；当前 PR 未引入 changeset
时使用 note 明确说明该事实，并说明合入目标分支后发布工作流仍会发布已完成版本准备但
registry 中尚不存在的版本。comment 必须包含稳定的隐藏 marker，只更新 Semifold 自己创建的 bot
comment；允许通过旧标题识别并迁移历史 Semifold comment，但不得更新其他 GitHub Actions 评论。
PR 引入的 changeset 必须通过 GitHub PR Files API 相对 base 检测，不依赖可能为 shallow 的本地
checkout；`.changes` 或 `.changesets` 根目录中非 removed 的 Markdown 文件视为当前 PR 引入或变更的
changeset，并处理全部分页。comment 必须单独列出这些 changeset，空状态以该集合为空为准，不能将
base 已有的 changeset 错误归因给当前 PR；全量 `ReleasePlan` 仍是版本表的事实来源。
comment 中标识计划计算位置的完整 commit SHA 必须以裸文本输出，不使用反引号包裹，使 GitHub
能够自动将其渲染为缩写且可点击的 commit 链接。

GitHub PR comment 的创建或更新失败保持非致命：已经生成的 `ReleasePlan` 与终端预览仍然有效，
`status` 不因评论副作用失败而返回非零。CLI warning 不得只显示 GitHub 客户端的顶层错误类别；当
Octocrab 返回结构化 GitHub API 错误时，必须展示失败的 create/update 操作、HTTP 状态以及 GitHub
返回的 message，并在存在时保留 documentation URL。403 响应还必须提供检查 workflow token 权限
和 fork PR 限制的可操作提示，但不能将所有 403 都断言为 fork 所致。其他客户端错误必须展示完整的
错误 source chain，同时继续遵守敏感信息不得进入终端输出的约束。只有单条错误事实时 warning 保持
单行；包含 API 错误、权限提示或文档链接时，首行只表达失败操作，后续事实使用缩进且带
`Error`、`Hint`、`Documentation` 语义标签的层级结构，不得将多条事实逐行顶格输出。

#### GitHub 操作诊断

所有 GitHub 调用必须保留失败操作和结构化诊断，覆盖客户端初始化、CI 发布 PR 查询/创建/更新、
status 评论和 PR 文件的查询/分页/写入、publish Release 创建与附件上传，以及 changelog PR 元数据查询。
共享不可变 `GitHubFailure` / `GitHubDiagnostic` 在现有 changelog 集成层定义，由 engine 保留并传递，
CLI 统一本地化展示操作、HTTP 状态、message、API validation errors 和 documentation URL；非 API
错误保留 source chain。不得输出请求头、token、完整请求或响应 debug dump；动态文本清除控制字符并
遮蔽当前 GitHub token。403 提示按操作区分 PR、评论、Release 权限，但不武断归因。
发布报告与 release preparation 保留诊断事实，既有 workflow output schema 不变；CI/publish 的致命
错误、评论写入与远程元数据查询的非致命降级、Release already_exists 幂等语义均不变。
测试策略：离线构造 API/嵌套客户端错误，覆盖 403、422 validation、5xx、脱敏和双语操作输出，
并检查 CI/status 各调用边界、发布失败报告和元数据降级路径。当前任务按用户要求禁止编译，
Rust 测试与 Cargo changeset status 验证留待空间恢复后执行，不将静态检查标记为运行验证。

#### CLI 参数与交互契约

CLI 不提供全局或命令级 `--non-interactive` 模式。交互提示只是参数缺省时面向终端用户的便利回退，
不是任何命令完成业务操作的必要输入通道。每个可交互输入都必须有语义等价、可组合且可在 help 中
发现的命令行参数；调用方提供完整参数后，命令不得读取 stdin、打开 prompt 或要求人工确认，从而
允许 CI/CD、无 stdin 子进程和受限 Agent 调度环境使用同一条命令。参数不完整且 stdin 或提示输出
不是终端时必须立即返回本地化错误，指出当前缺失输入及其等价参数，不能等待、采用隐藏默认值或
抛出底层终端错误。布尔确认必须提供互斥的显式正反参数；能够合法选择空集合或 `None` 的输入也必须
有显式参数，避免把“未传参”和“选择为空”混为一谈。

首个覆盖范围是所有仍依赖 `inquire` 的命令：

- `init` 使用既有 `--target`、重复 `--resolvers`、`--base-branch` 与 `--release-branch`，并增加
  `--no-resolvers`、`--default-tags` / `--no-default-tags`、`--github-actions` /
  `--no-github-actions` 和
  `--allow-non-root`。在仓库子目录运行时，只有缺少 `--allow-non-root` 才允许交互确认；
- `commit` 使用 `--name`、可重复的 `--summary`（短参数 `-m`）、重复
  `--package PACKAGE[=LEVEL]`、作为缺省 bump 的 `--level`，以及 `--tag` / `--no-tag`。每个 `-m`
  是一个独立 summary 段落，按参数顺序使用一个空行连接；单个 `-m` 保持原有单段内容。每个 package
  可以在参数中携带独立 bump；未携带时使用 `--level`，两者都缺失时才允许交互选择。参数路径与
  交互路径最终都只构造同一个
  `ChangesetDraft`，不能复制 changeset 校验或写入逻辑。

端到端测试必须关闭 stdin，并分别证明完整参数路径成功、缺少参数时快速失败且给出参数提示；TTY
交互行为只保留为输入收集适配层，不进入 engine。

#### 分层模板变量作用域

模板渲染必须在 `ReleasePlan` 完成和必需事实收集后执行。不同场景使用不同的
不可变视图，不共享无约束 map：

```rust
pub struct ReleasePackageContext<'a> {
    pub release: &'a ReleaseContext,
    pub package: ReleasePackageTemplateContext,
}

pub struct ReleasePackageTemplateContext {
    pub id: PackageId,
    pub name: String,
    pub ecosystem: Ecosystem,
    pub current_version: semver::Version,
    pub next_version: semver::Version,
    /// 迁移期对 `next_version` 的兼容别名。
    pub version: semver::Version,
    pub tag: String,
    pub path: Utf8PathBuf,
    pub private: bool,
}

pub struct ChangelogContext<'a> {
    pub package: ReleasePackageContext<'a>,
    pub changesets: Vec<PackageChangesetContext>,
    pub dependency_updates: Vec<DependencyUpdateContext>,
}

pub struct ChangesetContext {
    pub id: ChangesetId,
    pub summary: String,
    pub summary_paragraphs: Vec<Vec<String>>,
    pub commit: Option<CommitContext>,
    pub pull_request: Option<PullRequestContext>,
}

pub struct CommitContext {
    pub sha: String,
    pub short_sha: String,
    pub author: Option<String>,
    pub web_url: Option<String>,
}

pub struct PackageChangesetContext {
    pub changeset: ChangesetContext,
    pub section: String,
}

pub struct DependencyUpdateContext {
    pub package: PackageId,
    pub next_version: semver::Version,
}
```

现有 resolver `Changeset` 是包含 source path、全部 package 条目和清理方法的存储模型；
`ChangesetInput` 是只包含 `ChangesetId -> package bump` 的 planner 输入；二者都不直接暴露
给模板。`ReleasePlanContext.changesets` 只记录按 ID 稳定排序的已消费 changeset，不为 ID
额外包装 context，也不包含 summary 或远程元数据。

changelog 收集层从原始 `Changeset` 构造只读 `ChangesetContext`。一条 changeset 本身没有
唯一 tag；tag 和 bump 属于其中的 package 条目。对当前 package 收集 changelog 时，将 tag
通过 `[tags]` 解析为最终展示栏目 `section`，并构造 `PackageChangesetContext`；renderer
不读取原始 tag 配置。未指定或未配置 tag 时使用兼容栏目 `Changes`。bump 已由
`ReleasePlan` 消费且 renderer 不展示，因此不进入 changelog context。

`summary` 保留 changeset 原文；`summary_paragraphs` 是内容中立的结构化投影，以一个或多个
空行划分段落，并保留每段内物理行的顺序。内置默认模板将同一段的物理行用单个空格连接，避免
源文件为了行宽产生的编辑换行泄漏到 release note；自定义模板仍可选择使用原文或逐行结构。
`CommitContext.sha` 是完整 Git object ID，`short_sha` 是其确定性的
前 7 个字符，`author` 是 Git commit author name；不得把 author email 暴露给模板。

`ChangelogContext` 是一个实际发布 package 的完整聚合输入，而不是单条 changeset 的别名。
commit 和 pull request 与其来源 changeset 保持在同一个 `ChangesetContext` 内；依赖传播
产生的条目使用独立 `DependencyUpdateContext`。远程 PR 查询失败时收集层记录诊断并设置
`pull_request = None`，纯 formatter 不感知查询失败原因。

- release branch、release commit message 与 release PR title 模板只暴露 `release.*`；release PR
  body renderer 接收显式 `ReleasePullRequestContext`，其中 `release` 与三个模板引用同一个
  workspace `ReleaseContext`，changelog 是 version 规划后的应用层产物。
- version 与 changelog 中的包级视图同时暴露 `release.*` 与 `package.*`；
  `package.next_version` 和 `package.tag` 始终是该次版本计划的 package 事实。
- publish 的 pre-check、prepublish、publish、asset 和 GitHub Release 使用独立
  `PublishContext`，只暴露当前 manifest 可重建的 `package.*` 以及可选 repository/CI
  事实，不暴露或伪造 `release.*`。
- changelog 额外暴露 `changesets[*].changeset.*`、对应的 `section`、changeset 内可选的
  `commit` / `pull_request`，以及 `dependency_updates[*]`。单条 changeset 模板以
  `release.*`、`package.*`、`section` 和 `changeset.*` 为根变量；整体 changelog 模板以
  `release.*`、`package.*`、`changesets[*]`、`dependency_updates[*]` 和 renderer 生成的
  `sections[*].{name,entries}` 为根变量，其中 `entries[*]` 同时包含原始 `changeset`、`section`
  与单条模板生成的 `content`。
- workspace 级不提供 `release.version` 或 `release.tag`。需要共同版本时显式引用
  `release.plan.common_version`；需要具体 package 版本时按 `PackageId` 显式访问
  `release.plan.packages`。

MiniJinja 必须使用严格未定义变量模式，并在渲染后验证 branch ref、Git tag、
命令参数和 changelog release block。`common_version = None` 时引用该字段必须返回配置错误，
不渲染为空字符串。现有 pre-check 和 publish command 的 `package.name`、
`package.version`、`package.path` 与 `package.private` 字段在迁移期保持兼容。
version 阶段的 `package.version` 为 `package.next_version` 的兼容别名；publish 阶段的
`package.version` 是当前 manifest 版本。默认 package Git tag 保持当前
`<manifest-name>-v<next-version>` 约定，并以 `package.tag` 暴露；workspace 不从这些
package tag 中挑选一个作为自身 tag。

#### 分支配置与默认行为

`branches.base` 继续是基础分支字面量。`branches.release` 保持现有配置位置，但在
`ReleaseContext` 上以严格 MiniJinja 模板解析；不含模板语法的现有 `release` 值原样
渲染，因此单包和多包仓库均保持当前固定 release branch 行为。如需按计划
命名，可显式配置：

Semifold 当前发布工作流始终维护独立的 release branch，并从该分支向 `branches.base`
创建或刷新 Pull Request；将两者配置为同一分支不表示 trunk release 策略。CI 会强制更新
自动维护的 release branch，因此固定 `branches.release` 以及模板渲染后的最终分支都必须与
`branches.base` 不同。配置加载必须拒绝相同的固定分支，`init` 必须在写入任何文件前拒绝相同
输入；包含模板的配置还必须在 `ReleaseContext` 可用后重新校验渲染结果，并在准备版本文件、
切换分支或 push 之前失败。CLI 的 `--release-branch` help 与中英文配置文档必须明确这项约束，
不能让调用方把当前基础分支误认为发布分支默认值。

```toml
[branches]
base = "main"
release = "release/{{ release.plan.fingerprint }}"
```

具有共同版本的计划可以显式使用：

```toml
[branches]
base = "main"
release = "release/v{{ release.plan.common_version }}"
```

如需引用某个 package，必须在模板中显式使用它的 `PackageId`：

```toml
[branches]
base = "main"
release = 'release/v{{ release.plan.packages["semifold"].next_version }}'
```

该 package 不在本次实际发布集合时，严格模板渲染失败；Semifold 不配置或推断
“主 package”。多个独立 release branch / PR 不属于当前迭代；未来若存在明确产品
需求，必须先设计 scoped `ReleasePlan`、changeset 消费、跨 scope 依赖传播、共享文件
edit ownership 和独立恢复报告；不以恢复 `ReleaseUnit` 配置作为默认解法。

## 7. 生态适配器

### 7.1 接口

```rust
pub struct PackageLocation {
    pub id: PackageId,
    pub project_root: Utf8PathBuf,
    pub path: Utf8PathBuf,
}

pub struct ManifestDependency {
    pub manifest_name: String,
    pub kind: DependencyKind,
    pub requirement: Option<String>,
}

pub struct PackageInspection {
    pub id: PackageId,
    pub manifest_name: String,
    pub version: semver::Version,
    pub ecosystem: Ecosystem,
    pub path: Utf8PathBuf,
    pub publishable: bool,
    pub dependencies: Vec<ManifestDependency>,
}

pub struct EcosystemPlanInput<'a> {
    pub project_root: &'a Utf8Path,
    pub workspace_packages: &'a [PackageSnapshot],
    pub released_packages: &'a [PackageId],
    pub versions: &'a VersionMap,
}

pub trait EcosystemAdapter: Send + Sync {
    fn ecosystem(&self) -> Ecosystem;

    fn discover(
        &self,
        root: &Utf8Path,
    ) -> Result<Vec<PackageInspection>, AdapterError>;

    fn inspect(
        &self,
        package: &PackageLocation,
    ) -> Result<PackageInspection, AdapterError>;

    fn plan_edits(
        &self,
        input: EcosystemPlanInput<'_>,
    ) -> Result<Vec<FileEdit>, AdapterError>;
}
```

`PackageLocation.project_root` 是已规范化的绝对项目根，`path` 是项目根内的规范化相对
package path。adapter 从 manifest 只能解析依赖声明中的原生名称，因此 `discover()` 与
`inspect()` 返回 `ManifestDependency`，不得在 adapter 内猜测该名称对应的 Semifold
`PackageId`。应用层收集完整的 `PackageInspection` 后，按 `(Ecosystem, manifest_name)`
建立到稳定 `PackageId` 的唯一映射，仅将同一生态内唯一匹配的声明转换为
`PackageSnapshot.dependencies`；未匹配项视为外部依赖，重复 manifest name 则在构造
`WorkspaceGraph` 前报错。这样配置键可以与 manifest 名称不同，adapter 也无需读取全局
配置。

`EcosystemPlanInput.workspace_packages` 只包含当前 adapter 所属生态的完整
package 快照，`released_packages` 只包含本次实际发布的对应生态 package，并且
`versions` 仍是全工作区完整版本映射。批量输入允许 Rust adapter 在一个调用中合并共享
workspace manifest；不拥有共享 manifest 的生态也必须返回与 released package 输入顺序
无关的稳定 edit 集合。

### 7.2 职责边界

Adapter 可以：

- 读取并解析 Cargo.toml、package.json、pyproject.toml 等文件；
- 发现生态内包；
- 提取包名、版本、发布属性和内部依赖；
- 根据完整 `VersionMap` 生成新文件内容。

每个生态 adapter 只拥有本生态的原生 manifest：Rust 拥有 `Cargo.toml`，Python 拥有
`pyproject.toml`、`setup.cfg` 和 Python 源码版本文件，Node 拥有 `package.json`。
Python 或 Node binding 可以读取 Rust manifest 作为动态版本来源或构建输入，但不得写入
`Cargo.toml`。跨生态 package 默认保持独立版本序列；Rust 产物变化是否触发 binding
重新发布由显式依赖传播决定，不能通过复制 Rust 版本隐式实现。需要同版本发布时使用
后续显式 lockstep 版本规划策略；动态派生版本关系后续以显式 `version-source` 模型表达。

Rust manifest 与 Semifold TOML 配置必须使用保格式编辑器，保留无关的文本布局。`package.json` 则允许在完成 `serde_json::Value` 语义校验后使用启用 `preserve_order` 的序列化器规范化输出：对象键顺序必须保持，输出使用标准缩进并始终以一个换行结束。不得为 `package.json` 版本修改维护自定义字节级 JSON 解析或扫描器；JSON 结构、转义和边缘语法应完全由 `serde_json` 处理。

Rust adapter 规划依赖版本时必须同时处理 package manifest 中的 `dependencies`、
`dev-dependencies`、`build-dependencies`，以及 workspace 根 manifest 中的
`workspace.dependencies`。依赖使用别名时，以 `package` 字段解析真实 manifest name；
使用 `workspace = true` 时只修改共享的 `workspace.dependencies` 声明，不在成员
manifest 中插入重复版本。共享 workspace manifest 必须在一次 Rust 批量规划中合并所有
package version 与 dependency version 变化，同一路径只生成一个 `FileEdit`，结果不得依赖
release package 的遍历顺序。只有目标 package 的计划版本相对当前版本发生变化时才重写
对应依赖约束，避免将未变化的宽松约束无意义地规范化为精确版本。虚拟 workspace 根
没有可归属的 package 时，edit source 使用 `EditSource::WorkspaceDependencies` 并按
`PackageId` 稳定记录所有被更新的内部依赖。

Rust package 还可以通过 `package.version.workspace = true` 继承
`workspace.package.version`。该关系建模为 manifest 派生的共享 `VersionSource`，不是用户
配置的 `ReleaseUnit`，也不承载 branch、PR、tag 或 registry identity：

```rust
pub enum VersionSource {
    PackageManifest,
    Shared(VersionSourceId),
}

pub struct VersionSourceId {
    pub manifest: Utf8PathBuf,
    pub field: String,
}
```

同一个 `VersionSourceId` 的 package 形成隐式版本组。组内显式 changeset bump 取最高值；
任意成员触发版本变化时，所有成员进入 `ReleasePlan` 并获得同一个 `next_version`。原本没有
显式 bump 的成员记录共享版本传播原因，随后继续参与内部依赖传播。publishable package
正常发布；private package 同样参与版本计算和 manifest 事实更新，但在 `PublishPlan` 中以
private skip reason 跳过外部发布。只有 private 成员发生 changeset 也会推动共享版本，因而
可能使同组 publishable package 进入发布闭包。

同组 package 必须具有完全相同的 release channel 和一次性 `channel-bump`；不一致时规划
失败，不按 package 顺序选择配置。成员 manifest 中的 `version.workspace = true` 保持不变，
Rust adapter 仅对版本来源拥有者生成一次根 `Cargo.toml` 修改，将
`[workspace.package].version` 更新为组内统一目标版本。该 edit 必须稳定记录全部受影响
`PackageId`，并与根 manifest 中的 `workspace.dependencies` 修改合并为同一个 `FileEdit`。
缺少或无法解析 `workspace.package.version`、无法确定 workspace root、或同一来源得到不一致
当前版本时返回可处理错误，不得 panic。共享版本关系由 `PackageSnapshot.version_source` 提供给
`ReleasePlanner`，不从 `ReleaseContext`、分支或模板配置反向推导。

Node adapter 解析 `package.json` 时，缺失 `version` 必须视为 `0.0.0`，以支持未声明版本的模板项目；显式但无效的 `version` 仍必须报告解析错误。版本写入和 `FileEdit` 规划必须在缺失时插入目标 `version` 字段。

对于 manifest 内部依赖，adapter 必须同时提供依赖类别与原生版本约束。所有类别统一参与
`WorkspaceGraph` 排序；`ReleasePlanner` 的传播 allowlist 则按 6.3 节执行。首版只将 Rust
`[dependencies]` 视为约束感知的传播依赖：计划新版本不满足约束时，将依赖方自动加入发布闭包并规划
manifest 版本更新；约束仍满足时不单独发布依赖方。`dev-dependencies`、`build-dependencies`、
Node peer/optional 以及 Node.js、Python、C++ 的其他 manifest 边不自动传播；需要依赖发布即重新构建
或发布的关系必须用 `depends-on` 显式声明。

Adapter 不可以：

- 持有或读取全局 `Context`；
- 决定全局包顺序；
- 直接写文件；
- 执行子进程；
- 访问 registry 或 GitHub；
- 处理 dry-run。

#### C++ workspace 与内部依赖（首版）

阶段 0 为 CMake 项目采用可静态分析的最小规则，作为后续 `WorkspaceGraph` 的 fixture 基线：

- 工作区根为包含 `project(... VERSION ...)` 的根 `CMakeLists.txt`；
- 根文件中每个字面量 `add_subdirectory(path)` 声明一个直接成员；成员目录包含带版本的 `project(...)` 时，根项目与该成员均被发现为 package；
- 不解析变量、generator expression、下载或运行时生成的子目录；
- C++ adapter 迁移后递归跟随项目根内各 `CMakeLists.txt` 的字面量
  `add_subdirectory(path)`。中间目录可以只用于分组而不声明 package；每个包含
  `project(... VERSION ...)` 的可达目录均被发现为 package。遍历按规范化相对路径稳定
  排序并去重，指向项目根外的路径必须报错，不得读取或发现外部项目；
- 当成员项目的 `CMakeLists.txt` 以自身 `project` 名称作为第一个参数调用 `target_link_libraries(...)`，且后续参数中出现同一工作区内另一个 `project` 名称时，建立该内部依赖边；`PUBLIC`、`PRIVATE` 与 `INTERFACE` 在阶段 0 均只影响排序，不改变版本传播；
- 未匹配到上述静态形式的 CMake target 关系不推导为内部依赖，用户可在后续的可选 `depends-on` 配置中显式声明。

该规则只用于发现和依赖排序；版本写入、发布传播和跨生态依赖仍由后续 `WorkspaceGraph` 与 `ReleasePlanner` 定义。

### 7.3 跨生态依赖

某些跨生态依赖无法从 manifest 自动推导，例如 Node.js 包使用本地 Rust binding 产物。因此配置需要提供可选显式边：

```toml
[packages.node-binding]
path = "packages/node"
resolver = "nodejs"
depends-on = ["rust-core"]
```

该字段在新实现稳定后作为向后兼容的可选配置引入，不阻塞第一阶段迁移。

`depends-on` 中的值必须引用配置中的稳定 `PackageId`，允许跨生态引用。每条显式边转换为
`source = Config`、`kind = Unspecified`、无版本约束的内部依赖：

- 与 manifest 内部依赖共同参与拓扑排序、未知目标校验和依赖环检测；
- 依赖 package 发生任意版本发布时，触发 dependent 的 patch 发布，不受 dependent 所属生态限制；
- 不将该传播规则扩大到 Node.js、Python 或 C++ manifest 依赖；这些依赖种类的传播策略仍按阶段 4
  的独立任务定义；
- 新增 package 或执行 `config sync` 时不自动生成 `depends-on`，已有字段和顺序必须保留。

### 7.4 可扩展 ecosystem 插件（后续阶段）

Rust、Node.js、Python 与 C++ 之外的特定领域项目可能具有私有 manifest、版本来源和依赖规则。
后续应允许使用 JavaScript、Lua 或最终选定的受支持脚本运行时实现 ecosystem 插件，而不要求将
领域逻辑编译进 Semifold 主程序。该能力是内置 `EcosystemAdapter` 的扩展边界，不恢复旧的全局
resolver 职责。

脚本插件不能直接实现 Rust trait；host 应提供稳定、带 schema version 的序列化协议，将插件调用
映射为与 `EcosystemAdapter` 等价的 `discover`、`inspect` 和 `plan-edits` 能力。插件返回
`PackageInspection`、依赖声明和候选 `FileEdit`，由 host 继续执行路径规范化、PackageId 唯一性、
依赖图、文件 hash、冲突和项目根边界校验。插件不得直接写文件、运行发布命令、取得 registry 或
Forge 的宿主凭据，也不得创建 Forge release；publish hook、pre-check 与这些特权外部副作用仍由
engine 的既有端口统一编排。普通 HTTP 请求只能通过显式授权的网络 capability 发出。

为支持动态生态，现有闭集 `Ecosystem` / `ResolverType` 最终需要引入稳定的动态
`EcosystemId`，同时为四个内置生态保留固定 ID。插件注册、配置同步与 package 配置必须引用该
稳定 ID，不能依赖插件加载顺序。插件协议至少需要覆盖：

- 插件元数据、协议版本和生态 ID；
- package discovery、manifest inspection、依赖提取和版本来源；
- 基于完整 `VersionMap` 的确定性 edit 规划；
- 结构化诊断，包含插件、操作、package 和相关路径；
- 显式声明的文件读取范围及其他 host capability。

迁移分两步完成：领域层的 `PackageSnapshot`、release plan/context、config sync、publish context 与
`EcosystemAdapter` 先统一持有开放的 `EcosystemId`；旧公开名称 `Ecosystem` 只保留为类型别名，
四个旧 variant 名只作为兼容常量。插件注册表接入配置后，package 的 `resolver` 值与顶层
`[resolver.<id>]` key 都直接解析为开放的 `EcosystemId`；四个内置 canonical ID 固定为 `rust`、
`nodejs`、`python`、`cpp`，因此现有 TOML 不需要别名迁移。CLI 的 `ResolverType` 继续只表示四个
内置 resolver 的交互式选择，并显式转换为相同 `EcosystemId`。遇到尚未注册 adapter 的动态 ID
必须返回明确错误，不能回退到任一内置 resolver。

插件输入必须是不可变快照，输出必须可序列化和确定性排序。在相同输入及相同 capability 响应记录
下重复执行应产生相同结果；网络响应被视为显式外部输入，测试和重放必须使用可注入的 fetch backend。
host 不信任插件返回的路径、文件内容或依赖关系，所有结果都必须经过与内置 adapter 相同的验证。

首版插件运行时只支持 JavaScript，并通过纯 Rust 的嵌入式 Boa 执行单文件 ECMAScript module；不同时
支持 Lua，也不在 Semifold CLI 内置 TypeScript 转译。host 使用拒绝所有 import 的 module loader，
运行时产物必须是已经 bundle 的单文件 ESM，不能加载其他文件、远程 module 或原生 module。插件以
命名导出的 `metadata` 对象声明身份和能力，以默认导出的同步或异步函数作为统一入口，固定接收
`(request, host)` 并返回 response；schema-versioned request、response 与 host capability 均不得暴露
Boa 类型。`host` 是每次 operation 新建并冻结的 capability 对象，首版提供返回 Promise 的
`listFiles(pattern)` 与 `readText(path)`；未配置相应 host backend 时能力默认拒绝。读取 metadata 时
不创建文件 capability 且网络始终禁用，避免注册表发现阶段产生外部副作用。

插件开发体验由独立发布到 npm 的 TypeScript SDK 提供。SDK 只包含协议类型、构造辅助函数和允许的
host capability 声明，不把 Node.js 或浏览器 ambient API 伪装成可用能力。后续配套 Vite 插件负责把
插件编译为单文件 ESM，并在构建期拒绝残留 import、Node.js builtin、动态 module 加载以及 Semifold
运行时不支持的 Web API；构建检查是开发反馈，不替代 host 的运行时校验。

首个 SDK package 固定为 `@semifold/plugin-sdk`，源码位于 `packages/plugin-sdk`，以 ESM-only、零运行时
依赖的 npm package 独立发布。初始 manifest 版本使用 `0.0.0`，并由首个 minor changeset 进入
`0.1.0-rc.0`，避免在引入 release plan 前手工伪造已发布版本。SDK 以 ES2022 为最低输出目标，生成
JavaScript 与 declaration files；其源码和类型 fixture 的 TypeScript 编译环境只加载 ECMAScript 标准库，
显式排除 DOM 与 Node.js ambient 类型。SDK package 不负责 bundle 插件，也不成为 Semifold CLI 的
JavaScript runtime 依赖。package 的 `prepack` 必须从受版本控制的源码重建 `dist`，发布工作流必须先通过
仓库根 Bun workspace 和 `bun.lock` 安装锁定的 SDK 开发依赖，再由现有 Node.js resolver 在 package 目录
执行带 provenance 的公开 npm publish；不能依赖开发者提交生成目录或在 release job 中临时变更 lockfile。
npm package 必须声明与当前 GitHub 仓库一致的 repository metadata；release job 通过
`actions/setup-node` 配置 Node 24 和 npm registry，并直接使用该 Node 版本自带且满足 trusted publishing
最低版本要求的 npm CLI，不再单独安装或固定 npm。在
GitHub-hosted runner 上以既有 `id-token: write` 权限仅使用 OIDC。SDK 首版先由 maintainer 在本地
应用 version、以本机 npm 身份运行默认不创建 Forge release 的 `smif publish`，再将 version 提交合入 base
branch；对应 CD 通过 registry preflight 跳过已经发布的 npm 版本，只补建 GitHub Release。首发后再为
package 配置 trusted publisher，当前仓库的 workflow 不为 bootstrap 注入任何 npm token。

仓库自身的 JavaScript 开发工具链统一使用 Bun workspace：workspace 成员在根 `package.json` 声明，唯一
依赖锁文件为文本格式 `bun.lock`，安装、workspace script、文档构建和 SDK 测试均不得依赖 pnpm、
`pnpm-lock.yaml` 或 `pnpm-workspace.yaml`。所有需要 JavaScript 工具链的 CI/CD job 必须使用
`oven-sh/setup-bun` 并显式配置 `bun-version: canary`；不得依赖 action 默认版本，也不得固定为本地稳定版
`1.4.0`。Bun canary 作为有意采用的滚动工具链通道，第三方依赖版本仍由 `bun.lock` 固定。发布 job 继续
单独安装 Node 24，仅用于 npm trusted publishing。此仓库工具链决策不改变 Node adapter 对外部 npm 与
pnpm workspace 的解析兼容性，也不改变 npm registry 的 package 格式和发布协议。

SDK 的所有 wire types 使用与 Rust 协议相同的 kebab-case JSON 字段，并以 `V1` 后缀表达 schema 边界；
不得增加一层自动 camelCase 转换。公共构造 API 首版固定为 `definePluginMetadata`、`definePlugin`、
`createPluginDiagnostic`、`createPluginSuccess` 与 `createPluginFailure`：metadata helper 固定 schema version
和三个必需 operation，并对 read pattern 去重排序；entrypoint helper 只提供类型约束，不吞掉异常；response
helper 根据 request 写入 operation，failure helper 必须自行构造至少一条 error diagnostic。SemVer、生态 ID、
路径、hash、诊断归属和 response/request operation 一致性仍由 Rust host 做最终验证，SDK 不复制一套可能
漂移的安全校验器。

schema v1 的 JSON wire types 必须从 `semifold-resolver` 中使用 serde 的同一组 Rust 协议模型通过 `ts-rs`
确定性生成，不能继续在 SDK 中手工复制字段、tag 或 optional 规则。Rust 协议是结构的唯一事实来源；生成器
必须从 Rust 常量输出字面量 `schema-version` 与 operation 常量，并将 `EcosystemId`、`PackageId`、SemVer 的
透明 JSON 表示投影为 TypeScript `string`。生成产物提交到 SDK 源码树，但不得在 npm `prepack` 中调用 Cargo；
独立的 `--check` 模式和 CI drift 检查必须在 Rust 协议变化而生成产物未同步时失败。跨语言 JSON fixture 继续
保留，用于验证 serde 运行时形状，而不是由静态类型生成替代。

ts-rs 的声明文本不是提交格式。生成器必须把原始声明通过 SDK 固定版本的 Biome 与仓库根配置规范化后，
再写入 `src/generated/protocol.ts`；`--check` 必须对同样规范化后的候选内容与已提交文件做字节比较，避免
格式化本身被误报为协议漂移。Biome 不可用、格式化进程失败或未返回有效 UTF-8 时，生成与检查都必须失败，
不得退回未格式化输出。SDK 的只读检查脚本和 CI 必须同时覆盖协议 drift、Biome format/lint 与现有类型、
运行时测试；写入式生成命令应一次产生可直接提交且再次检查幂等的文件。

自动生成只覆盖 wire schema，不覆盖 SDK 的运行时 helper、Boa capability ambient 声明或面向插件作者的
请求/输出关联类型。生成模型作为内部 raw wire layer；SDK 公共类型从该层派生，并递归施加只读属性，使嵌套
对象、数组和 map 保持现有不可变输入契约。`schema-version` 在 `V1` 类型中必须保持字面量 `1`，不能退化为
允许任意 `number`。operation-specific request、success output 和 diagnostic helper 类型应通过 `Extract`、
`Pick` 等 TypeScript 类型运算从生成 union 派生，避免再次手写同一协议结构。

SDK 只为实际注册的 Boa 能力声明 ambient globals。文件能力通过入口参数中的 `PluginHostV1` 暴露，包含
`listFiles(pattern)` 与 `readText(path)`；网络侧只声明当前 Boa runtime 真正支持的 `fetch` request init、
response、headers 子集，以及不包含 `searchParams` 等未实现成员的 `URL` 子集。它不加载完整 `lib.dom`，
也不声明 `window`、`document`、`process`、`Buffer`、任意文件系统或子进程 API。SDK 的类型 fixture 必须在
无 DOM/Node ambient 的条件下覆盖 metadata、三类 request/output、诊断、failure、文件读取、fetch 与 URL；
运行时测试必须验证 helper 生成的 JSON 与 Rust schema v1 形状一致，并验证 metadata 的稳定排序和冻结行为。

运行时不提供任意文件系统、环境变量或子进程访问。插件只能调用 host 显式注入的 `listFiles`、
`readText` 与标准形态的 `fetch` capability。文件路径使用 `/` 分隔、相对项目根目录且不得包含空路径、
绝对路径、反斜杠、`.` 或 `..` 分段。`listFiles` 只接受与 metadata `read-patterns` 中某一项逐字相同的
glob，避免以不可靠的 glob 子集推断扩大授权；`readText` 只接受匹配至少一个已声明 glob 的规范相对
路径。两项能力每次调用都要经过项目根目录、解析后的符号链接边界、普通文件类型、文件数量和字节
预算校验；匹配到越过项目根目录的符号链接必须报错，不能静默忽略。`listFiles` 的结果去重后按 UTF-8
字典序排序，且 glob 默认不隐式匹配以 `.` 开头的路径段。网络默认拒绝；插件配置必须声明允许的精确
HTTPS origin。host 自定义 fetch client 对初始 URL 和每次重定向执行协议与 origin 校验，禁用 reqwest
自动重定向和 ambient proxy；跨 origin 重定向即使目标也被授权，仍必须移除 authorization、cookie 和
proxy authorization 等敏感请求头。具体 transport 使用独立、可复用的 Tokio runtime 驱动异步 reqwest，
不依赖调用方是否已有 Tokio runtime，并通过自定义 DNS resolver 只把全部解析结果均为公网地址的目标
交给连接器，避免域名、IP literal 和 DNS rebinding 绕过地址边界。插件不能读取 ambient credential；
需要认证的值必须由未来独立的命名 secret capability 显式授权，首版不实现该能力。

首版限制固定而非可配置：插件源文件最大 1 MiB，Boa 每次 operation 使用全新 Context，循环迭代上限
为 10,000,000、递归深度上限为 256，并使用 Boa 的 VM stack limit；单个读取文件最大 4 MiB，一次
operation 通过 `readText` 累计返回最大 32 MiB，`listFiles` 累计最多返回 10,000 个路径，重复调用同样
计入累计预算。单个网络请求最长 10 秒、请求体最大 8 MiB、响应体最大 8 MiB，最多跟随 5 次重定向；
一次 operation 最多 8 个请求、最多 4 个并发请求，请求体与响应体分别累计最大 32 MiB。重定向产生的
每次实际 HTTP exchange 都计入请求次数、并发数和 body 预算。Boa 当前没有可与
QuickJS heap limit 等价的硬内存上限，因此 host 还必须限制 request、response、诊断和所有 capability
载荷大小；在允许未审查的第三方插件前，运行时必须迁移到独立进程以取得 wall-clock 与内存硬隔离。
循环、递归、栈、capability 或载荷预算超限均转换为结构化插件诊断，host 不复用失败的 Context。

首版只加载仓库内、相对项目根目录的单文件 `.js` 插件，不支持 URL、registry 或运行时下载。配置
以稳定 `EcosystemId` 注册插件路径；`sha256` 可选，存在时加载前必须严格校验 digest，不存在时仍
计算并记录本次实际加载内容的 SHA-256，但不把它当作跨次加载的内容锁。网络授权缺省为空：

```toml
[plugins."com.example.engine"]
path = "plugins/engine.js"
# sha256 = "可选的 64 位小写十六进制摘要"
allowed-origins = ["https://api.example.com"]

[packages.engine]
path = "engine"
resolver = "com.example.engine"
```

注册表按 `EcosystemId` 排序并拒绝重复 ID 或覆盖 `rust`、`nodejs`、`python`、`cpp` 四个内置 ID。
`EcosystemId` 使用小写 ASCII 点分段，每段以字母开头、以字母或数字结尾，中间允许数字与连字符；
最长 128 字节。插件元数据包含 SemVer plugin version，
配置了 `sha256` 时可执行内容以 digest 为最终锁定事实；未配置时 repository commit 是唯一外部
内容边界。首版 package version model 仅支持 SemVer，与现有 `VersionMap` 和 release planner 保持一致。

host 与插件只交换 UTF-8 JSON。首版协议 schema version 为整数 `1`，覆盖 metadata、discover、
inspect、plan-edits、不可变 workspace/version 快照、候选 file edit 和结构化诊断。host 只接受
自己支持的 schema version；同一 schema 可以增加可选字段，删除字段、改变字段语义或类型必须提升
schema version。plugin version 与 protocol schema 独立演进。协议 DTO 位于 adapter 边界，不能把
脚本 runtime 类型泄漏到 `semifold-core` 或 engine application service。

通过 digest 和 metadata 校验的 loaded plugin 直接实现现有 `EcosystemAdapter`，不再增加第二套
resolver 抽象。plugin 的文件和网络 capability 在注册时绑定规范化项目根；adapter 每次调用仍必须
确认调用方项目根规范化后与绑定根完全一致，并只向脚本传递 `project-root = "."`，避免插件把宿主
绝对路径当作稳定输入。workspace package、dependency、released package 与候选 edit 在进入协议或
返回领域层前统一按稳定键排序；首版插件版本模型限定为 SemVer，因此 `encode_version()` 使用严格
SemVer 的规范文本。插件操作失败时，`AdapterError` 必须保留完整结构化诊断；成功输出仍需经过生态
归属、PackageId 唯一性、规范相对路径、符号链接边界、edit source 引用、重复目标和当前文件 SHA-256
校验，hash 计算必须流式执行，不能按不可信候选文件大小分配等量内存。

## 8. Crate 和模块边界

建议使用四层结构，并为 Node.js 分发增加一个不承载领域逻辑的 N-API 适配 crate：

```text
crates/
├── semifold-core/
│   ├── package.rs
│   ├── dependency.rs
│   ├── changeset.rs
│   ├── workspace.rs
│   ├── release_plan.rs
│   └── versioning.rs
│
├── semifold-ecosystems/
│   ├── rust.rs
│   ├── node.rs
│   ├── python.rs
│   └── cpp.rs
│
├── semifold-engine/
│   ├── loader.rs
│   ├── config_sync.rs
│   ├── config_editor.rs
│   ├── planner.rs
│   ├── executor.rs
│   ├── publisher.rs
│   └── ports.rs
│
├── semifold-napi/
│   └── lib.rs
│
└── semifold/
    ├── cli/
    ├── github/
    ├── mcp/
    └── main.rs
```

### 8.1 `semifold-core`

纯领域层，不依赖：

- `git2`；
- `reqwest`；
- `octocrab`；
- `clap`；
- `inquire`；
- 真实文件系统和子进程。

可以依赖 `semver`、`serde`、`thiserror` 和轻量路径类型。

### 8.2 `semifold-ecosystems`

包含各生态 manifest 解析和内容重写。它依赖 core，但 core 不依赖它。

现有 `semifold-resolver` 建议渐进演化为这一层，而不是一次性删除后重写。

### 8.3 `semifold-engine`

应用层负责：

- 加载项目；
- 组合生态 adapter；
- 构建 `WorkspaceGraph`；
- 构建 `ReleasePlan` 和 `PublishPlan`；
- 验证并执行文件修改；
- 通过 port 调用 Git、HTTP、命令和 Forge 实现。

### 8.4 `semifold`

仅负责入口和表现：

- Clap 参数解析；
- 交互提示；
- 本地化文案；
- 日志与终端输出；
- GitHub Actions 编排；
- MCP transport 和工具参数映射。

CLI 命令不再直接读写 manifest 或实现版本计算。

### 8.5 `semifold-napi`

`semifold-napi` 是 `semifold` CLI 的薄 Node-API 分发适配层，不提供新的领域 API。它只把 Node.js
传入的参数序列映射为与独立二进制相同的 CLI 执行入口，并返回进程退出码。当前工作目录、环境变量、
stdin、stdout、stderr、TTY 检测和信号仍属于调用它的 Node.js 进程，不得复制终端渲染、配置加载、
应用服务或本地化逻辑。

binding 使用稳定的 N-API 8 ABI。同步调用会占用 Node.js 主线程，但 `@semifold/cli` 是一次只执行一个
命令的专用 CLI 进程，该行为与原生二进制一致；不得为了异步 JavaScript API 把交互提示、MCP stdio
transport 或进程级副作用迁移到 worker。独立 Rust 二进制继续存在，N-API 只是新增的分发入口。

## 9. 项目加载与 `Context` 拆分

对于要求项目已初始化的命令，使用完整类型：

```rust
pub struct Project {
    pub root: Utf8PathBuf,
    pub changeset_dir: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    pub config: Config,
}
```

`Project::load()` 不应将错误吞掉为 `None`：

```rust
pub enum ProjectLoadError {
    RepositoryNotFound,
    ChangesetDirectoryNotFound,
    ConfigNotFound,
    NonUtf8Path { path: PathBuf },
    ConfigInvalid { path: Utf8PathBuf, source: ConfigError },
    RepositoryOpenFailed { path: Utf8PathBuf, source: GitError },
}
```

`init` 使用独立的发现结果：

```rust
pub struct ProjectLocation {
    pub root: Utf8PathBuf,
    pub existing_config: Option<Utf8PathBuf>,
}
```

GitHub 环境、Git 仓库和 dry-run 均不是 `Project` 数据的一部分。

repository root、changeset directory 或 config path 无法无损转换为 UTF-8 时，返回
`ProjectLoadError::NonUtf8Path` 并保留原始 `PathBuf`；不得使用 lossy conversion，也不得 panic。

`Project` 负责项目加载，一次 release 的动态事实以及包或 changelog 的模板数据则
分层表示为 `ReleaseContext`、`ReleasePackageContext`、`PublishContext` 和
`ChangelogContext`。`Project` 不是
模板 context；首版不将其 root、配置路径或完整 `Config` 序列化给模板。所有
context 必须是不可变数据快照；Git、Forge、文件系统和 resolver 等能力通过 engine 的
依赖注入提供，而不放回 context。

## 10. 副作用边界

应用层通过小型 port 表达需要的外部能力：

```rust
pub trait FileSystem {
    fn read(&self, path: &Utf8Path) -> Result<Vec<u8>, FsError>;
    fn write_atomic(&self, path: &Utf8Path, data: &[u8]) -> Result<(), FsError>;
    fn remove(&self, path: &Utf8Path) -> Result<(), FsError>;
}

pub trait CommandRunner {
    fn run(&self, command: &CommandSpec) -> Result<CommandOutput, CommandError>;
}

pub struct CommandSpec {
    pub executable: String,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: Utf8PathBuf,
    pub phase: CommandPhase,
    pub stdout: StdioPolicy,
    pub stderr: StdioPolicy,
    pub run_in_dry_run: bool,
}

#[async_trait]
pub trait RegistryClient {
    async fn version_exists(&self, check: &RegistryCheck) -> Result<bool, RegistryError>;
}

pub trait GitRepository {
    fn is_clean(&self) -> Result<bool, GitError>;
    fn commit_for_path(&self, path: &Utf8Path) -> Result<Option<CommitInfo>, GitError>;
}

#[async_trait]
pub trait ForgeClient {
    async fn pull_request_for_commit(
        &self,
        commit: &CommitId,
    ) -> Result<Option<PullRequestInfo>, ForgeError>;

    async fn create_release(
        &self,
        release: &ForgeRelease,
    ) -> Result<ForgeReleaseOutcome, ForgeError>;

    async fn upload_asset(
        &self,
        release: &ForgeReleaseId,
        name: &str,
        content: Vec<u8>,
    ) -> Result<(), ForgeError>;
}
```

`ForgeRelease` 包含 repository、package tag、标题、changelog body 与 prerelease 标记；
`ForgeReleaseOutcome` 显式区分已创建和已存在，并在可上传 asset 时返回稳定的
`ForgeReleaseId`。engine 通过 `FileSystem` 读取 asset bytes，Forge adapter 不读取本地路径。
`CommandSpec.run_in_dry_run` 来自配置字段 `dry-run`；它不是命令自身的模拟参数，而是全局
dry-run 下调用 `CommandRunner` 的显式许可。

不需要在第一次迁移中抽象所有 IO。优先抽出 `FileSystem`、`CommandRunner` 和 `RegistryClient`，因为它们直接决定 plan/apply 的可测试性。

## 11. 文件修改与失败策略

`version` 执行流程应为：

1. 加载并规划所有变更。
2. 验证依赖图、新版本和文件修改冲突。
3. 根据 `expected_hash` 确认文件未在规划后改变。
4. 将新内容写入临时文件。
5. 以尽可能原子的方式替换目标文件。
6. 写入 changelog。
7. 成功后删除 changeset。
8. 最后执行非事务性 post-version 命令。

每个 `FileEdit` 必须显式声明其规划时的目标状态：已有文件使用源内容哈希，首次创建文件则要求目标在应用时仍不存在。执行器必须在写入任何临时文件前验证全部前置条件；不得将“不存在”隐式当作空文件，否则并发创建的 changelog 可能被无意覆盖。

Post-version 子进程不可能真正纳入跨进程事务。在执行前应尽可能完成所有静态验证。命令失败时保留已写入的文件和 changeset，不尝试不可靠的自动回滚；`ApplyReport` 清楚报告已完成文件、失败命令和未消费 changeset，供用户修复命令后重试。

生产代码不得使用会 panic 的 `unwrap()` 处理外部输入、文件系统、配置或领域查询结果；这些失败必须通过类型化错误或可传播错误返回。只有已由类型、构造器或同一函数内穷尽分支证明的内部不变量可以使用 `expect()`，且消息必须说明该不变量；不得把可恢复失败标记为不变量。测试代码中的断言性使用不属于运行时失败路径。

`--dry-run` 不调用 Semifold 文件写入器，不创建外部 package release，也不上传 asset；
它仍执行只读 registry preflight，并仅对 `run_in_dry_run = true` 的命令调用
`CommandRunner`。这类命令可能具有用户定义的副作用，因此结构化报告必须明确列出实际执行
与跳过的命令；该行为是配置的显式授权，不得由 engine 猜测命令是否安全。

## 12. Changelog 设计

Changelog 生成应分为两部分：

1. 可选元数据收集：从 Git 和 Forge 获取 commit、PR 和 author；
2. 纯模板渲染：根据 changeset、版本和元数据生成当前 package 的 release block。

renderer 接收 `ChangelogContext` 的只读模板视图，不应持有全局 `Context`、
`git2::Repository` 或自行创建 `Octocrab`。GitHub PR 元数据查询失败时记录诊断并降级为
不含 PR 信息的 changelog，不中断 `version`；该策略必须在收集层实现，不能成为模板渲染的
隐式行为。

配置新增可选 workspace 级 changelog 模板：

```toml
[changelog]
template = """
## {{ package.next_version }}

{% for section in sections %}
### {{ section.name }}

{% for entry in section.entries %}
{{ entry.content }}
{% endfor %}
{% endfor %}
"""

changeset-template = """
- {{ changeset.summary }}
"""
```

配置模型为：

```rust
pub struct ChangelogConfig {
    pub template: Option<String>,
    pub changeset_template: Option<String>,
}
```

`template` 渲染一个 package 本次发布的完整 release block；`changeset-template` 渲染一条
`PackageChangesetContext`。首版只支持配置内的字符串模板，不支持模板文件，也不支持 package
级覆盖。任一字段缺省时使用对应的内置默认模板；两者都缺省时，生成结果必须与引入模板能力前
字节级兼容。`smif init` 必须显式写出 `[changelog]` 以及当前内置的 `template` 和
`changeset-template`，使新用户可以直接从生成的配置中发现并修改模板；加载旧配置或用户删除任一
字段时仍使用同一份内置模板作为运行时 fallback。初始化输出和运行时 fallback 必须复用同一模板
来源，不能维护两份可能漂移的默认值。

renderer 必须分两阶段执行：先以 `release`、`package`、`section` 和 `changeset` 为根变量渲染
每一条 changeset，再按 `section` 和 changeset ID 的既有稳定顺序形成仅存在于 renderer 内部的
`RenderedSection` / `RenderedChangeset`，最后以 `release`、`package`、原始 `changesets`、
`dependency_updates` 和 `sections` 渲染整体模板。整体模板通过
`sections[*].entries[*].content` 消费单条模板结果；如果用户选择直接遍历原始 `changesets`，
则视为显式绕过 `changeset-template`。这些中间结果不是领域事实，不进入 `semifold-core`，也不
写回 `ChangelogContext`。

两个模板必须在一次 `prepare_release` 中以 MiniJinja strict undefined 模式各编译一次并复用于
全部 package。任一模板编译失败、引用未定义字段、单条结果为空、整体结果为空或包含 Semifold
保留 marker 时，必须在产生任何文件副作用前返回包含 package、changeset（适用时）、模板种类和
模板位置的结构化错误。`version --dry-run` 同样编译、渲染并校验模板，但仍不收集远程 PR 元数据。

Semifold 继续拥有 `CHANGELOG.md` 文档骨架和历史内容，用户整体模板只拥有当前 release block。
为避免把 `##` 标题结构作为隐含模板约束，写入层必须在用户模板结果外添加不可见的稳定边界：

```markdown
<!-- semifold:release version=1.2.0 -->
用户模板生成的任意内容
<!-- semifold:release:end -->
```

模板不得生成或覆盖上述保留 marker。缺失 changelog 时，Semifold 仍创建包含
`# Changelog` 根标题的文件；已有文件必须包含唯一可定位的根标题，新 block 插入根标题之后并
保留全部历史。为同时满足缺省配置的字节级兼容，只有任一用户模板已配置，或目标文件已经包含
Semifold marker 时，新 block 才添加 marker；从未启用模板且没有 marker 历史的文件继续使用原格式。
文件一旦包含 marker，后续即使移除自定义模板也继续为新 block 添加 marker，避免最新版本重新变成
不可可靠定位的无标记内容。`read_latest_changelog` 优先从第一组完整 marker 读取版本和正文，返回正文时排除
marker；没有 marker 的旧文件继续回退到现有 `# Changelog` 加首个 `## ` 标题的解析方式。
marker 缺失配对、嵌套、重复开始或版本无效必须作为 changelog 解析错误，不得猜测恢复。

内置默认 changeset 模板保持现有行为：每个用户 changeset 渲染为独立 Markdown 列表项。
只有单段 summary 时，连续列表项保持紧凑。summary 以一个或多个空行划分段落；每个续段以
一个空行与前段分隔，并缩进四个空格，使其保持在同一列表项内。同一段内因源文件行宽产生的
物理换行必须规范化为单个空格，不能在 release note 中产生缩进续行；空行不生成只含缩进空格的
伪段落。最后一个续段后保留一个空行，再渲染下一个列表项。commit 与 PR 元数据附在首段末尾。例如：

```markdown
- First line ([#42](https://example.com/pull/42) by @author)

    Second line

    Third line

- Another changeset
```

依赖传播而自动加入发布闭包的 package 仍会被发布，不能生成空 changelog。规划器必须为它记录
`ReleaseReason::DependencyPropagation { dependency, next_version }`；内置默认整体模板将该原因
渲染为独立的 `Dependencies` 区段，例如：

```markdown
### Dependencies

- Update semifold-resolver to 0.4.0-alpha.0.
```

该条目不伪装成用户 changeset，也不使用 changeset 的 changelog tag。只有实际会发布的 dependent 才生成该记录；dev、build 或其他不影响发布产物的依赖传播策略由 adapter 规则单独定义。

## 13. `config` 指令设计

### 13.1 目标

Semifold 的运行时配置仅支持 TOML；`config.json` 不再是可加载或可保存的 Semifold 配置，发现
该文件时必须返回明确的 `UnsupportedConfigFormat`，并提示迁移为 `config.toml`。生态 manifest
中的 JSON（例如 `package.json`、`vcpkg.json`）不受此限制。所有 TOML 配置字段统一使用
kebab-case。所有 Rust 字段名中包含下划线的
配置键必须映射为连字符形式，例如 `dry-run`、`extra-env`、`extra-headers`、`pre-check`、
`post-version`、`channel-bump`、`depends-on` 与 `changeset-template`。本次格式切换不为 snake_case 字段提供 alias
或迁移兼容；仓库自带配置、初始化模板、文档示例和测试 fixture 必须同时切换。Rust 内部字段名
不受影响，模板变量与 locale key 也不属于配置字段，不随此规则改名。

`init` 只负责首次创建 Semifold 配置。工作区在后续开发中新增、删除、移动或重命名包时，不应要求用户重复执行 `init`，也不应覆盖已经手工维护的发布命令、assets、version mode 和跨生态依赖。

初始化 Rust resolver 时，内置 `post-version` 命令为 `cargo generate-lockfile`，并允许在 dry-run 中执行。
该默认命令不得附加 `--offline`，以便依赖解析在本地索引或缓存不足时正常访问 registry；用户仍可在生成配置后按项目需要自行加入离线参数。

新增配置同步入口：

```text
smif config sync
```

该命令重新扫描已配置的生态系统，将实际工作区与 `.changes/config.toml` 中的 `[packages]` 进行比较，并使用 `toml_edit` 对原始 TOML 文档进行局部修改。

核心要求：

- 保留注释、字段顺序、空行和用户自定义字段；
- 只修改需要同步的 package table；
- 默认不静默删除配置；
- 支持只检查、不写入；
- 同一输入重复执行结果不变；
- `init` 和 `config sync` 复用同一套包发现服务。

### 13.2 CLI 语义

第一版建议提供：

```text
smif config sync
smif config sync --check
smif config sync --prune
smif config sync --resolver rust --resolver nodejs
```

参数语义：

| 参数 | 行为 |
| --- | --- |
| 无参数 | 扫描配置中已经启用的 resolver，展示差异并应用安全更新 |
| `--check` | 只检查工作区与配置是否一致；存在差异时返回非零退出码，适合 CI |
| `--prune` | 删除已经无法发现的 package 配置；默认只报告，不删除 |
| `--resolver <type>` | 只同步指定 resolver；可以重复传入 |

全局 `--dry-run` 继续有效，但语义与 `--check` 不同：

- `--dry-run` 输出将要应用的完整 `ConfigPlan`，但不写文件，退出码不表示配置漂移；
- `--check` 用于断言配置已经同步，发现任何需要修改的内容时返回非零退出码。

只更新 `.changes/config.toml` 或 `.changesets/config.toml`。如果当前项目使用 JSON 配置，所有运行
入口和配置命令都返回明确的 `UnsupportedConfigFormat`；`config migrate` 不读取或重写 JSON，用户
必须先将配置转换为 TOML。

### 13.2.1 旧配置迁移

为使仓库能够从旧的 `version-mode` 过渡到 `channel`，并为 kebab-case 配置切换提供显式迁移，
提供独立且不执行 workspace discovery 的入口：

```text
smif config migrate
smif config migrate --check
```

迁移只处理 `[packages.*]` 中的版本通道字段，并使用 `toml_edit::DocumentMut` 保留其余内容：

- `version-mode = "semantic"` 或缺省语义模式迁移为缺省 `channel`（删除旧字段，不写入 `channel = "stable"`）；
- `version-mode = { pre-release = { tag = "alpha" } }` 迁移为 `channel = "alpha"`；
- 已使用 `channel` 的 package 保持不变；同一 package 同时设置 `channel` 与 `version-mode` 时停止并报告冲突；
- 将已知 snake_case 配置字段原位重命名为对应 kebab-case，包括 `version_mode`、
  `channel_bump`、`depends_on`、`pre_check`、`post_version`、`extra_headers`、`extra_env` 和
  `dry_run`；字段值、注释、table/array-of-tables 顺序及其他未知字段保持不变；
- 旧的 `[resolver.*.pre-check]` 仅含 `url` 且缺少 `type` 时补充 `type = "http"`；运行时 loader
  不接受缺少判别字段的旧结构；
- 同一 table 同时存在 snake_case 与目标 kebab-case 字段时停止并报告冲突，不覆盖任一值；
- loader 不接受 snake_case alias；旧配置必须先运行 `config migrate`，该迁移支持不等于运行时兼容；
- JSON 配置返回明确的不支持错误；
- `--check` 在存在可迁移条目时返回非零且不写文件；全局 `--dry-run` 只报告将要迁移的条目，退出成功；
- 成功迁移后再次运行不得产生 diff。

`config migrate` 是配置引导命令：CLI 只发现仓库、changeset 目录和配置文件路径，随后直接从原始
TOML 生成迁移计划，并对迁移后的内容执行严格 `Config` 反序列化验证；不得在该命令之前通过
`ProjectLocation::load()` 解析旧配置或执行 workspace discovery。除 `init`、`mcp` 及该迁移入口
外，其余项目命令仍先严格加载完整 `Project`。迁移应用继续使用原子写回。

普通 CLI 命令在严格加载 TOML 配置时收到 `ResolveError::InvalidConfig`，必须完整展示该底层错误并
追加本地化恢复建议，引导用户先尝试运行 `smif config migrate`。该建议只表示配置可能使用旧契约，
不能宣称迁移一定能够修复任意 TOML 语法或手工配置错误。配置文件缺失、读取 I/O 失败、项目路径错误
以及不受支持的 JSON 配置不得显示该建议，因为 `config migrate` 无法解决这些问题。迁移命令自身继续
直接报告迁移规划或验证错误，不递归附加相同建议。恢复建议另起一行，并以 `✘` 开头，使其与底层
诊断正文形成清晰分隔。

该命令是格式迁移工具，不替代 `config sync`，也不自动将 stable package 显式改写为 `channel = "stable"`。

### 13.2.2 发布通道管理

为显式管理已配置 package 的发布通道，提供以下命令：

```text
smif config channel set alpha --package semifold
smif config channel set alpha --package semifold --package semifold-resolver
smif config channel set alpha --all
smif config channel set alpha --package semifold --bump preserve
smif config channel clear --package semifold
```

`set` 只接受非空的命名通道；`stable` 是保留值，恢复 stable 必须使用 `clear`。命令必须指定一个或多个 `--package <PackageId>`，或显式指定 `--all`，两者不能同时使用。未知 package 是错误，避免因拼写失误产生无效配置。

`set` 修改目标 package 的 `channel` 字段，并可通过可选的
`--bump <preserve|patch|minor|major>` 记录一次性 `channel-bump` 转换策略。未指定
`--bump` 时保持现有行为，由 changeset 的最高 bump 决定首次进入通道的稳定基准。
`preserve` 保留当前稳定基准，`patch`、`minor` 和 `major` 则显式覆盖该基准的提升级别。
例如 `0.1.0` 进入 `alpha` 时分别得到 `0.1.0-alpha.0`、`0.1.1-alpha.0`、
`0.2.0-alpha.0` 和 `1.0.0-alpha.0`。该策略仅写入本次 `set` 需要覆盖的目标 package；
已处于请求 channel 的 package 不会因 `--bump` 单独发生变化。

`channel-bump` 只在 package 当前为 stable 且下一次 `version` 首次进入配置的命名通道时生效。
成功应用 manifest、changelog 和 post-version 后，`version` 从对应 package table 删除该字段；
dry-run、规划失败、文件应用失败或 post-version 失败都不得消费它。通道内后续发布仍只推进序号。

`clear` 删除 `channel` 和未消费的 `channel-bump`，使 package 回到缺省 stable 状态，且不接受
`--bump`。二者都使用 `toml_edit::DocumentMut` 与原子写回，保留目标 table 的其他字段、注释和所有非目标 package。无实际变化时不得写入文件。

全局 `--dry-run` 只输出将修改的 package 而不写入；`--check` 断言目标已处于请求状态，存在需要修改的 package 时返回非零。JSON 配置不受支持。

当 `channel set` 实际需要修改的目标中包含 Node.js package，且其内置 `nodejs` resolver 配置了
未显式携带 `--tag` 或 `--tag=<value>` 的 `npm publish` 命令时，CLI 必须警告这些 package，并提示
在发布前为 npm 命令补充目标 channel 对应的 dist-tag。该检查在 `--check` 和全局 `--dry-run`
下同样执行，但只提供诊断：不得自动改写 publish 命令，不检查或覆盖用户已经显式设置的 tag，
也不改变自定义命令。原因是一个 resolver 配置可能被不同 channel 的 package 共享，静态写入当前
channel 会改变其他 package 的发布语义。

### 13.3 同步计划

与 release 流程相同，配置更新也采用 Plan/Validate/Apply：

```rust
pub struct ConfigSyncPlan {
    pub config_path: Utf8PathBuf,
    pub prune_missing: bool,
    pub added: Vec<DiscoveredPackage>,
    pub missing: Vec<ConfiguredPackage>,
    pub renamed: Vec<PackageRename>,
    pub moved: Vec<PackageMove>,
    pub conflicts: Vec<ConfigConflict>,
    pub warnings: Vec<ConfigSyncWarning>,
}
```

`prune_missing` 是计划的执行语义而不是 CLI 临时参数：只有完整 resolver 扫描成功且显式请求
`--prune` 时为 `true`。`apply_config_sync()` 只消费计划本身，不得从入口层另取 prune 状态。

首版领域输入与分类结果使用以下结构：

```rust
pub struct ConfiguredPackage {
    pub id: PackageId,
    pub ecosystem: Ecosystem,
    pub path: Utf8PathBuf,
}

pub struct DiscoveredPackage {
    pub id: PackageId,
    pub ecosystem: Ecosystem,
    pub path: Utf8PathBuf,
}

pub struct PackageRename {
    pub from: PackageId,
    pub to: PackageId,
    pub ecosystem: Ecosystem,
    pub path: Utf8PathBuf,
}

pub struct PackageMove {
    pub package: PackageId,
    pub ecosystem: Ecosystem,
    pub from: Utf8PathBuf,
    pub to: Utf8PathBuf,
}

pub enum ConfigConflict {
    AmbiguousMatch {
        configured: Vec<ConfiguredPackage>,
        discovered: Vec<DiscoveredPackage>,
    },
    ResolverChanged {
        configured: ConfiguredPackage,
        discovered: DiscoveredPackage,
    },
}

pub struct ChangesetReference {
    pub changeset: ChangesetId,
    pub packages: BTreeSet<PackageId>,
}

pub enum ConfigSyncWarning {
    ChangesetReferencesRenamedPackage {
        changeset: ChangesetId,
        from: PackageId,
        to: PackageId,
    },
}
```

`ConfigSyncPlanner` 是纯函数式领域服务，接收 config 路径、已配置 package 快照、完整 discovery 快照和未消费 changeset 引用。package path 在进入 planner 前必须已转换为相对项目根目录的规范化 UTF-8 路径；路径规范化失败属于应用层输入错误，不伪装为同步冲突。planner 不读取文件、不决定 `--prune`，也不修改配置。

输出使用 `missing` 而不是 `removed`：它只表示配置项未被发现；是否删除由应用阶段结合完整扫描状态和 `--prune` 决定。rename 命中旧 package id 的未消费 changeset 时生成 `ChangesetReferencesRenamedPackage` warning，但不修改 changeset。所有结果按 package id、路径、生态和 changeset id 稳定排序。同一输入无论迭代顺序如何都必须产生相同计划。

discovery 从 manifest name 生成的 `PackageId` 只是新增和 rename 的默认建议，不得覆盖已经配置的稳定
身份。匹配前先识别同一生态内的重复建议；这代表 manifest name 本身有歧义，直接分类为
`AmbiguousMatch`。随后严格按以下顺序匹配：

1. 第一轮匹配相同 ecosystem 与相同规范化 path。默认建议唯一且 package id 不同时分类为 rename；
   当同一默认建议被不同生态复用时，保留现有配置的 `PackageId`，不产生 rename；
2. 对仍未匹配的 discovery 建议检查全局唯一性。多个未匹配 package 竞争同一建议，或建议已被第一轮
   匹配的另一个 package 占用时，分类为 `AmbiguousMatch`，不得自动生成 namespace；
3. 第二轮在剩余项中匹配相同 `PackageId` 与相同 ecosystem，path 不同时分类为 move；
4. 相同路径或相同 `PackageId` 的 ecosystem 改变分类为 `ResolverChanged`。

任一轮存在多对一或一对多候选时，将全部相关项分类为 `AmbiguousMatch`，不得同时把它们报告为
added 或 missing。剩余 discovery 项为 added，剩余 config 项为 missing。

执行流程：

```text
读取原始 config.toml
→ 解析 Config 与 toml_edit::DocumentMut
→ 使用 ecosystem adapters 重新发现包
→ 计算 ConfigSyncPlan
→ 检查冲突和删除策略
→ 对 DocumentMut 做局部修改
→ 原子写回 config.toml
```

领域层只计算 `ConfigSyncPlan`。`toml_edit::DocumentMut` 属于配置文件 adapter，不进入 `semifold-core`。

### 13.4 包匹配规则

同步不能只按包名匹配，否则重命名会被误判为“一删一增”，并丢失原配置中的手工字段。建议按以下优先级匹配：

1. resolver 与规范化 package path 完全相同：视为同一个包；manifest name 改变且默认
   `PackageId` 建议唯一时识别为 rename，跨生态同名导致建议冲突时保留已配置 ID；
2. `PackageId` 相同：path 改变时识别为 move；discovery 的默认 `PackageId` 从 manifest name 派生；
3. 名称和路径均不匹配：视为新增和缺失；
4. 多个包同时命中同一候选：产生冲突，不自动修改。

规范化路径必须：

- 相对于项目根目录；
- 使用统一分隔符；
- 移除 `.` 和可安全消解的 `..`；
- 不跟随项目根目录外的符号链接。

应用层提供共享的 `PackagePathNormalizer`，供 workspace 快照桥接、`init` 和 `config sync` 使用：

```rust
pub fn normalize_package_path(
    project_root: &Path,
    package_path: &Path,
) -> Result<Utf8PathBuf, PackagePathError>;
```

`project_root` 必须是已存在的绝对目录；`package_path` 可以是绝对路径或相对于项目根目录的路径。实现先做不访问文件系统的词法规范化，再检查最深的已存在祖先路径：如果符号链接解析后的祖先位于项目根目录之外则拒绝该输入，但返回值仍保留项目内的词法相对路径，不改写为符号链接目标。尚不存在但位于项目根目录内的 package path 可以被规范化，以便配置中的 missing package 参与同步。项目根 package 统一表示为 `.`，序列化路径统一使用 `/` 分隔符；无法表示为 UTF-8 或逃逸项目根目录时返回结构化错误。

Rename 时应尽量移动原有 TOML table，而不是重新创建，以保留：

- `channel`；
- `assets`；
- `depends-on`；
- 未来新增的未知字段；
- table 前后的注释和装饰信息。

如果存在尚未消费且引用旧包名的 changeset，rename 必须产生警告。第一版不自动重写 changeset，避免修改用户提交的变更记录。

### 13.5 新增、删除和更新策略

#### 新增包

新发现的包加入 `[packages]`：

```toml
[packages.example]
path = "packages/example"
resolver = "rust"
```

稳定通道为默认值时不显式写入 `channel`。`assets`、`depends-on` 等无法从 manifest 推导的字段不自动生成。

新条目必须采用确定性顺序插入：在 `[packages]` 内按 `PackageId` 排序新增条目，但不重排现有条目，避免产生大面积无意义 diff。

### 13.6 版本通道

包的发布状态由可选的 `channel` 表达，而不是由 `VersionMode` 或 `pre-release` 概念表达：

```toml
[packages.semifold]
path = "crates/semifold"
resolver = "rust"
channel = "alpha"
```

`channel` 的规则如下：

- 缺省 `channel` 与 `channel = "stable"` 等价，均表示稳定通道；
- `stable` 是保留关键字，不能作为自定义通道；
- 任何其他非空值都是命名发布通道，例如 `alpha`、`beta`、`next`、`nightly` 或 `internal.2026`；
- `[tags]` 和 changeset 中的 tag 仅用于 changelog 分类，绝不决定版本通道或版本号；
- 包从 stable 首次进入某个命名通道时，changeset 的最高 `major`、`minor` 或 `patch` 决定该通道周期对应的下一稳定基准版本；该基准版本不需要额外的手工设定。

领域层使用下列抽象：

```rust
pub enum ReleaseChannel {
    Stable,
    Named(String),
}
```

配置加载时，缺省值和 `stable` 都解析为 `ReleaseChannel::Stable`；其余值解析为 `ReleaseChannel::Named`。配置同步对新 stable package 省略 `channel`，但不应因无关同步操作删除用户显式写入的 `channel = "stable"`。

`ReleaseChannel` 是发布流程概念，不是 SemVer 的 `Prerelease` 概念。核心仅处理通道状态与序号推进；为保持现有 `ReleasePlan` 的统一排序和序列化，它暂以 SemVer 形状保存过渡版本值，但这个值不是 manifest 的最终版本字符串。`EcosystemAdapter::encode_version()` 必须在 `plan_edits()` 前将其验证并编码为所属生态的版本文本，所有 package version 与内部依赖版本编辑都必须使用该结果：

| 生态 | `channel = "alpha"` 的一种编码 | `channel = "post"` 的一种编码 |
| --- | --- | --- |
| Rust / Node（SemVer） | `1.0.0-alpha.1` | `1.0.0-post.1` |
| Python（PEP 440） | `1.0.0a1` 或项目约定格式 | `1.0.0.post1` |
| CMake / vcpkg | 不支持命名通道；`project(VERSION)` 只接受稳定数字版本 | 不支持 |

Rust 和 Node adapter 直接使用合法的 SemVer 文本。Python adapter 将 `alpha.N`、`beta.N`、`rc.N` 和 `post.N` 分别编码为 PEP 440 的 `aN`、`bN`、`rcN` 和 `.postN`，并在 inspection 时将这四种 PEP 440 格式还原为领域的过渡值；其他命名通道在 Python 规划时失败。CMake adapter 拒绝任何含 prerelease 的过渡值，避免生成 CMake 不接受的 `project(VERSION)`。因此，`channel` 的字符串不在 core 中按各生态规范限制；adapter 必须在规划阶段验证其能否表示并在无法表示时返回结构化错误。

版本通道的状态完全由当前版本和当前 `channel` 决定，不能依赖已经被 version 命令消费的 changeset。规则如下：

- stable 包首次进入命名通道时，先按 changeset 计算下一稳定基准，再生成 `<base>-<channel>.0`；例如 `0.2.16 + major + alpha` 生成 `1.0.0-alpha.0`；
- 当前包已处于相同命名通道时，任意非 `Unchanged` changeset 都只推进通道序号；例如 `1.0.0-alpha.0 + major` 生成 `1.0.0-alpha.1`。这代表同一个待发布稳定版本周期内的后续变更，不重复提升 stable 基准；
- 切换到另一个命名通道时保留稳定基准并将序号重置为 `.0`；例如 `1.0.0-alpha.2 → beta` 生成 `1.0.0-beta.0`；
- `Unchanged` 不生成新版本；
- 从命名通道回到 stable 时移除 prerelease 后缀并发布当前基准，例如 `1.0.0-alpha.2 → 1.0.0`。

stable 包的 major/minor/patch 计算保持现有语义。上述规则不能从 changelog tag 推断，adapter 仍负责将领域通道编码为所属生态的合法版本。

#### 删除包

默认行为只报告：

```text
- package old-core is configured but no longer exists
  run 'smif config sync --prune' to remove it
```

只有 `--prune` 才删除 table。以下情况即使指定 `--prune` 也必须拒绝自动删除：

- 多个 resolver 扫描发生错误，无法确认包确实已删除；
- 包路径暂时不可访问；
- 存在匹配歧义；
- 本次使用 `--resolver` 排除了该包所属 resolver。

#### 移动和重命名

路径移动只更新 `path`。重命名更新 package table key，并保留原 table 内容。若新名称已经存在，产生冲突并停止写入。

#### Resolver 变化

同一路径从一种生态变成另一种生态时，不应自动覆盖 `resolver`。这通常意味着项目结构发生重大变化或发现器判断错误，必须作为冲突要求用户确认。

### 13.7 使用 `toml_edit` 保格式更新

当前 `save_config()` 将强类型 `Config` 整体重新序列化，会丢失用户原有格式选择和部分注释。`config sync` 不应调用该路径。

建议增加独立组件：

```rust
pub struct TomlConfigEditor {
    path: Utf8PathBuf,
    document: toml_edit::DocumentMut,
}

impl TomlConfigEditor {
    pub fn load(path: &Utf8Path) -> Result<Self, ConfigEditError>;
    pub fn validate(&self) -> Result<Config, ConfigEditError>;
    pub fn apply(
        &mut self,
        plan: &ConfigSyncPlan,
        prune_missing: bool,
    ) -> Result<(), ConfigEditError>;
    pub fn render(&self) -> String;
}
```

编辑时直接操作：

```rust
let packages = document["packages"]
    .as_table_mut()
    .ok_or(ConfigEditError::MissingPackagesTable)?;
```

实现要求：

- 加载后先反序列化成强类型 `Config` 完成语义验证；
- 使用 `DocumentMut` 修改原文档，而不是从 `Config` 重新序列化；
- 只修改 `[packages]` 下需要同步的 table；`[branches]`、`[release]` 与其他发布策略配置完全保留；
- 保留未知字段，确保旧版 Semifold 不会抹掉新版或插件写入的配置；
- 写回前再次从修改后的文档反序列化并验证；
- 使用临时文件与 rename 原子替换；
- 文件内容未变化时不执行写入。

`apply` 在存在任何 `conflicts` 时必须在修改文档前返回错误。`prune_missing = false` 时仅应用 `added`、`renamed` 与 `moved`，`missing` 保持为报告项且不得删除 table；`prune_missing = true` 时删除 `missing` 对应的 package table。CLI 只可在完整扫描成功、未排除任何 resolver 且计划无冲突时传入 `true`，随后使用临时文件和 rename 原子写回。

### 13.8 与 `init` 的关系

`init` 和 `config sync` 应共享：

- resolver registry；
- package discovery；
- package path 规范化；
- 默认 package 配置生成；
- 冲突诊断。

迁移期间由应用层提供统一发现入口，隐藏 resolver factory 与领域 ecosystem 的映射：

```rust
pub struct PackageDiscovery {
    pub resolvers: Vec<ResolverType>,
    pub packages: Vec<DiscoveredPackage>,
}

pub struct PackageDiscoveryService {
    registry: ResolverRegistry,
}

impl PackageDiscoveryService {
    pub fn discover(
        &self,
        project_root: &Path,
        resolvers: &[ResolverType],
    ) -> Result<PackageDiscovery, PackageDiscoveryError>;
}
```

resolver 选择先按类型稳定排序并去重。服务通过 registry 创建现有 resolver，调用发现接口，将 manifest name 作为默认 `PackageId` 建议，并使用共享 `PackagePathNormalizer` 生成规范化路径。`PackageDiscovery.packages` 按 `PackageId`、ecosystem 和 path 稳定排序；重复建议保留在发现快照中。`ConfigSyncPlanner` 可用相同 ecosystem 与 path 将跨生态同名结果绑定到已有稳定 ID。首次 `init` 生成默认配置时，对跨生态重复建议使用 6.1 节定义的 ecosystem 前缀和确定性数字后缀；`config sync` 中尚未配置的多个新增 package 继续分类为多义冲突，不擅自改变增量配置中的稳定 ID。

一次 discovery 只有“完整成功”或“失败”两种结果：任一所选 resolver 的 glob 遍历、manifest 读取、package 解析或路径规范化失败时，整个调用返回结构化错误，不得返回看似完整的部分快照。未选择的 resolver 不属于扫描范围；应用层据此禁止在部分 resolver 模式下 prune 其他生态的配置。现有 `resolve_all` 中记录 warning 后跳过损坏 package 的路径必须改为传播错误，避免把扫描失败误判为 package 已删除。

`plan_config_sync` 的应用层桥接通过 `ConfigSyncScope` 选择 resolver：未指定 `--resolver` 时选择配置中全部已启用的 resolver；显式选择必须是已启用 resolver 的子集，否则返回 `ResolverNotEnabled`。scope 将这些生态的 `[packages]` table 转换为 `ConfiguredPackage`，调用统一 discovery，并把未消费 changeset 转换为 `ChangesetReference` 后交给 `ConfigSyncPlanner`。未被本次 resolver 范围覆盖的配置项不得进入 `missing`。CLI 的显式 `--resolver` 只改变 scope，不复制快照转换或匹配逻辑；只有 scope 覆盖全部已启用 resolver 时才允许 `--prune`。

区别仅在于：

- `init` 从空配置生成初始文档和 CI 模板，并对单包与多包仓库都生成兼容现有行为的固定 `branches.release`；
- `config sync` 从现有 `DocumentMut` 生成并应用增量修改；
- `init --force` 也不应继续成为日常同步工作区的方式。

长期可将 `init` 的配置生成实现为“创建最小文档后应用一次完整 `ConfigSyncPlan`”，避免两套包发现和配置生成逻辑再次分叉。

### 13.9 输出示例

```text
Configuration drift detected:

  + crates/new-adapter       rust
  ~ packages/node            moved from bindings/node
  → old-python               renamed to python-core
  - crates/legacy            missing (kept; use --prune to remove)

Updated .changes/config.toml
```

无变化时：

```text
Configuration is up to date.
```

输出结构应来源于 `ConfigSyncPlan`，终端渲染、JSON 输出和未来 MCP 工具可以复用同一个结果。

## 14. CLI、CI 与 MCP

### 14.1 CLI

CLI 仅负责：

1. 解析参数；
2. 调用 application service；
3. 渲染结果和错误。

```rust
pub struct SemifoldService<D> {
    deps: D,
}

impl<D: Dependencies> SemifoldService<D> {
    pub fn plan_init(
        &self,
        location: &ProjectLocation,
        options: InitOptions,
    ) -> Result<InitPlan, AppError>;
    pub fn apply_init(
        &self,
        plan: InitPlan,
    ) -> Result<InitReport, AppError>;
    pub fn ensure_clean_worktree(
        &self,
        project: &Project,
        allow_dirty: bool,
    ) -> Result<(), AppError>;
    pub fn create_changeset(
        &self,
        project: &Project,
        draft: ChangesetDraft,
    ) -> Result<ChangesetId, AppError>;
    pub fn create_changeset_with_mode(
        &self,
        project: &Project,
        draft: ChangesetDraft,
        mode: ExecutionMode,
    ) -> Result<ChangesetId, AppError>;
    pub fn get_changesets(
        &self,
        project: &Project,
        id: Option<&str>,
    ) -> Result<Vec<ChangesetRecord>, AppError>;
    pub fn create_changeset_idempotent(
        &self,
        project: &Project,
        draft: ChangesetDraft,
        mode: ExecutionMode,
    ) -> Result<ChangesetMutationResult, AppError>;
    pub fn update_changeset(
        &self,
        project: &Project,
        id: &str,
        expected_revision: &FileHash,
        draft: ChangesetDraft,
        mode: ExecutionMode,
    ) -> Result<ChangesetMutationResult, AppError>;
    pub fn delete_changeset(
        &self,
        project: &Project,
        id: &str,
        expected_revision: &FileHash,
        mode: ExecutionMode,
    ) -> Result<ChangesetMutationResult, AppError>;
    pub fn plan_config_sync(
        &self,
        project: &Project,
        options: ConfigSyncOptions,
    ) -> Result<ConfigSyncPlan, AppError>;
    pub fn apply_config_sync(
        &self,
        plan: ConfigSyncPlan,
    ) -> Result<ConfigSyncReport, AppError>;
    pub fn plan_release(&self, project: &Project) -> Result<ReleasePlan, AppError>;
    pub async fn prepare_release(
        &self,
        project: &Project,
        release: ReleasePlan,
        options: ReleaseExecutionOptions,
    ) -> Result<ReleaseApplyPlan, AppError>;
    pub fn apply_release(
        &self,
        plan: ReleaseApplyPlan,
        mode: ExecutionMode,
    ) -> Result<ApplyReport, AppError>;
    pub async fn plan_publish(
        &self,
        project: &Project,
        options: PublishOptions,
    ) -> Result<PublishPlan, AppError>;
    pub async fn publish(
        &self,
        plan: PublishPlan,
        mode: ExecutionMode,
    ) -> Result<PublishReport, AppError>;
}
```

`init` 的询问与 embedded workflow asset 读取仍属于 CLI；用户选择完成后，CLI 将目标目录、
resolver、branch、tag 和可选 workflow 模板组成 `InitOptions`。应用服务统一完成 package
发现、默认 resolver 配置构造、配置序列化和 workflow 渲染，返回包含全部目标目录与文件内容
的不可变 `InitPlan`；`apply_init()` 只消费该计划创建目录并写入文件。这样 `init` 与
`config sync` 复用同一个 `PackageDiscoveryService`，入口层不直接构造配置或操作文件。

### 14.2 CI

CI 编排仍然可以处理 release branch、commit、push 和 Pull Request，但必须调用同一 `SemifoldService`，不再直接复用 CLI 模块中的具体实现函数。

### 14.3 MCP

MCP 服务不应在每个工具调用中重新构建全局 `Context`，也不应依赖 `set_current_dir()` 修改进程全局状态。

MCP handler 持有固定仓库范围的 `ProjectLocator` 与无状态 application service，不在 transport 启动前
严格加载 `Project`。stdio 握手完成后，每次工具调用从 locator 重新加载当前配置与 changeset 目录，
使旧配置、暂时无效的配置、分支切换或 MCP 生命周期内的配置变更成为该次调用的结构化 tool error，
而不是服务器启动失败、缓存陈旧或进程退出。`-C` 保持兼容，公开长参数使用不暗示修改进程目录的
`--project-root`，旧 `--cd` 作为 alias 保留。

首版 MCP 只声明 tools capability，不声明未实现的 resources、prompts 或 logging。server info 版本来自
实际 Semifold package version，stdio 的 stdout 只承载 MCP frame；日志和启动诊断只能写 stderr。
全局 `--dry-run` 在 MCP 中表示整个 session 不应用 changeset 写入：所有写工具仍执行定位、解析、输入与
revision 验证并返回 `planned`，但不得创建、替换或删除文件，不能继续被静默忽略。

MCP 对 changeset 只提供四个结构化 CRUD 工具：

- `get_changeset` 接受可选 `id`；有 id 时读取单条，没有 id 时按 id 稳定排序返回全部；
- `create_changeset` 接受 name、summary 及逐 package 的 package、bump 与可选 tag；不得使用“第一个 tag”
  等隐藏默认。目标不存在时创建，同名内容完全相同时幂等返回 `existing`，内容不同时返回
  `CHANGESET_CONFLICT`；
- `update_changeset` 以固定 id 完整替换 packages 与 summary，不提供局部 patch 或 rename；调用方必须
  提供最近一次 get/create 返回的 SHA-256 `revision`；
- `delete_changeset` 删除固定 id，同样必须提供 `revision`。名称错误时显式 delete 后重新 create，
  不提供独立 rename 工具。

同一个 MCP server 内的 create/update/delete 串行执行，防止两个并发调用同时通过同一 revision 检查；
锁中毒必须恢复为可继续服务的状态。create 使用不覆盖已有目标的原子写入，update 使用原子替换，任何
失败都不得留下被当作 changeset 扫描的临时 `.md` 文件。

get/create/update/delete 的成功结果都返回 JSON `structuredContent`，包含稳定 status、changeset DTO 和
revision；失败结果使用 `isError = true` 的结构化对象，至少包含稳定 `code`、本地化 `message` 与相关
id/revision 详情。输入、项目定位、配置解析、文件读取、领域验证、revision 冲突和文件写入失败都属于
tool-level error，不得作为 JSON-RPC internal error 使调用方丢失信息，也不得让 stdio server 退出。
handler 边界捕获可 unwind 的意外 panic 并转换为 `INTERNAL_ERROR` tool result；生产实现自身不得使用
`unwrap()`、无说明 `expect()` 或索引假设。客户端正常关闭 stdio、操作系统终止、OOM 或 panic=abort
等进程外/不可 unwind 终止不属于可恢复请求边界。

CLI 与 MCP 提交 changeset 时都先将各自的参数或交互结果映射为 `ChangesetDraft`。应用层统一负责
名称规范化、重复文件检查、package/tag 校验和写入；入口层不得直接构造或提交 resolver
`Changeset`：

从磁盘加载 changeset 时必须执行与创建路径一致的领域校验。当前生成格式使用位于文件开头和
YAML front matter 结尾的两个独占一行的 `---`；加载器同时兼容不含开头 marker、仅以一个 `---`
结束 front matter 的旧格式。除此之外的缺失、重复或出现在正文中的独立分隔符必须拒绝。front
matter 必须包含至少一个已配置 package，分隔符之后必须包含非空 summary；空 package 集合和空
summary 必须在 changeset 加载阶段返回结构化错误，不得延迟到 release planning 或 changelog 渲染
阶段。`status` 可以继续只构造纯 `ReleasePlan`，但其输入必须已经通过上述完整校验。

```rust
pub struct ChangesetDraft {
    pub name: String,
    pub packages: Vec<ChangesetPackageInput>,
    pub summary: String,
}

pub struct ChangesetPackageInput {
    pub package: PackageId,
    pub bump: BumpLevel,
    pub tag: Option<String>,
}
```

### 14.4 GitHub Actions 工作流输出

部分项目的构建、签名、制品组装或发布只能在 CI/CD 的后续 job 中完成，无法由本地 CLI 的
prepublish/publish hook 在一个进程内闭环。`smif version` 与 `smif publish` 后续应支持将经过筛选
的发布事实写入 GitHub Actions 提供的 output 文件，使 workflow 可以把 Semifold 的计划和执行
结果传递给后续 step 或映射为 job output。

该能力不得直接序列化内部 `ReleaseContext`、`PublishContext` 或 error 类型。application 层应从
这些模型派生稳定、带 `schema-version` 的 workflow DTO，再由 GitHub Actions output adapter
负责写入。这样内部 context 可以继续演进，而 workflow 消费方只依赖显式兼容契约。

version 输出至少需要表达本次 plan fingerprint、release branch，以及每个实际进入发布计划的
package 的稳定 ID、manifest name、当前版本、目标版本、tag 和相对路径。它来源于 version 使用的
同一个 `ReleaseContext`，不得在 changeset 被消费后重新推断。publish 输出至少需要表达每个
package 的 ID、名称、版本、tag、相对路径、private 状态，以及最终 succeeded、skipped、failed
或 not-started disposition；发生部分失败时，结构化 `PublishReport` 中已经完成和未启动的状态仍
应可供后续恢复步骤消费。

workflow DTO 不得包含 registry header、环境变量值、命令配置、token、author email 或其他秘密。
非 GitHub Actions 环境默认不写入任何额外文件，也不改变现有终端输出。output writer 是独立外部
端口，不能进入 core，且必须使用 GitHub Actions 支持的安全多行写入格式，避免换行或用户内容
破坏 output 边界。

具体 output key、JSON schema、是否需要显式 CLI 开关、dry-run 是否写 planned output、publish
失败时 output 写入失败与原始发布失败的优先级，以及 schema 的兼容周期，在实现前单独确定。

### 14.5 Node.js N-API CLI 分发

npm 的公开安装入口为 `@semifold/cli`，同时声明 `smif` 与 `semifold` 两个 bin。JavaScript bin 只把
`process.argv.slice(2)` 传入 native binding，并把返回值设置为进程退出码；参数解析、帮助、版本、
业务输出和错误仍由 Rust CLI 统一负责。

`@semifold/cli` 是仓库中唯一维护、进入 Bun workspace、Semifold package 配置和 changeset 的 npm
交付单元。它的 `package.json` 通过 napi-rs v3 的 `napi.binaryName` 与 `napi.targets` 声明以下六个
构建 target：

- `aarch64-apple-darwin`；
- `x86_64-apple-darwin`；
- `aarch64-unknown-linux-gnu`；
- `x86_64-unknown-linux-gnu`；
- `aarch64-pc-windows-msvc`；
- `x86_64-pc-windows-msvc`。

主包不得捆绑所有平台产物，也不得在 postinstall 中从 GitHub 下载可执行文件。napi-rs 生成的 loader
通过精确版本的 optional dependencies 选择对应平台包；平台包必须声明 npm `os`、`cpu` 与适用时的
`libc` 约束，并只包含对应 native binding、package metadata 与 npm 自动包含的 README。首版 Linux
与现有原生发布矩阵一致，只支持 glibc；不宣称 musl 支持。主包要求 Node.js 20 或更高版本，发布仍
使用 Node.js 24 与 npm provenance。

napi-rs 平台包不是仓库源码 package：不得提交 `npm/` 目录，不得把这些临时 package 加入 Bun
workspace、`.changes/config.toml` 或 changeset。发布 CI 使用 `napi create-npm-dirs` 从根包配置临时
生成它们，下载六平台构建产物后由 `napi artifacts` 组装。根包的 `prepublishOnly` 使用
`napi prepublish -t npm --no-gh-release`，由 napi-rs 把所有平台包同步到根包版本、写入精确版本的
`optionalDependencies` 并先发布平台包；外层 `npm publish` 随后发布 `@semifold/cli`。禁用 napi-rs
的 GitHub Release 副作用，由 Semifold 继续统一管理 Forge release。

`@semifold/cli` 通过 `depends-on = ["semifold"]` 建立与 Rust CLI 的拓扑关系，并在发布前校验两者
版本一致。napi-rs 生成的 loader 包含生成时的根包版本，因此 Node.js resolver 的通用 post-version
hook 只调用各 package 可选的 `semifold:post-version` script；`@semifold/cli` 用该 script 在 manifest
版本写入后重新生成并提交 loader 与类型声明，其他 Node.js package 没有该 script 时保持无操作。
当前锁定的 napi-rs CLI 会在 prepublish 阶段拒绝缺失 target artifact，但 CI 仍必须在进入任何 npm
publish 生命周期前显式验证临时平台目录完整、每个目录恰好包含预期 `.node`，并通过
`npm pack --dry-run --ignore-scripts` 检查根包和平台包内容；任一缺失、重复或命名不匹配必须提前
使任务失败，不能把完整性门禁隐式委托给依赖工具的当前行为。

JavaScript bootstrap 使用 napi-rs 生成的 loader，不维护第二份 target 选择和 native package 加载
实现。正常 CLI 文案继续由 Rust 与 `rust-i18n` 管理；binding 尚未加载时的环境诊断由生成的 loader
负责。

## 15. 错误模型

`anyhow` 适合 CLI 最外层补充上下文，但 core、ecosystems 和 engine 内应返回分层错误：

```text
DomainError
├── DuplicatePackage
├── UnknownDependency
├── DependencyCycle
└── InvalidVersionTransition

AdapterError
├── ManifestNotFound
├── ManifestParse
├── UnsupportedManifestShape
└── EditConflict

AppError
├── ProjectLoad
├── Domain
├── Adapter
├── FileSystem
├── Git
├── Registry
└── Command
```

CLI 负责将这些错误转换成本地化用户消息，不应让翻译宏进入 core。

生产代码不得依靠 panic 表达错误或内部控制流。除依赖库内部与进程资源耗尽等无法由
Semifold 控制的情况外，Semifold 自身生产路径不得使用 `unwrap()`、`expect()`、`panic!()`、
可能越界的集合索引或未经验证的切片；外部输入、配置、文件系统和领域查询失败必须进入上述
结构化错误边界。能够由构造过程证明长度一致的集合也应优先使用 `zip` 等无索引迭代，避免后续
重构破坏隐式不变量。测试中的断言式索引和 panic 不进入生产构建，可继续用于表达测试失败。

## 16. 测试策略

### 16.1 领域单元测试

- changeset 的 bump level 合并；
- stable 与命名通道的版本计算；
- 依赖传播；
- 确定性拓扑排序；
- 依赖环诊断；
- 同一组输入产生完全相同的 `ReleasePlan`。

### 16.2 Ecosystem fixture/golden tests

每个生态建立 `fixtures/`：

```text
fixtures/rust/
├── single-package/
├── workspace/
├── workspace-dependencies/
├── optional-dependencies/
└── unpublished-package/
```

测试内容：

- 发现的 `PackageSnapshot`；
- 解析的内部依赖；
- 给定 `VersionMap` 后产生的 `FileEdit`；
- 应用 edit 后的完整 golden manifest；
- 原始文件中与版本无关的格式和字段不被破坏。

### 16.3 Executor 测试

- hash 冲突时拒绝写入；
- 任一预写入失败时不删除 changeset；
- dry-run 没有任何写操作；
- 计划与最终文件完全一致；
- post-version 命令的顺序和失败报告。

### 16.4 CLI 端到端测试

- `smif status`、`smif version --dry-run` 与实际 `smif version` 展示相同计划；
- 未初始化项目返回稳定错误码；
- dirty Git 工作区策略；
- 发布 preflight 跳过已存在版本；
- CI 流程调用同一 release plan。

### 16.5 Config sync 测试

- 新增包只增加一个 table，且第二次执行无 diff；
- 默认不删除缺失包，`--prune` 明确删除；
- 扫描失败时 `--prune` 不产生删除；
- rename 和 move 保留 assets、version mode、未知字段与注释；
- 匹配歧义和 resolver 变化产生冲突；
- `--check` 在漂移时返回非零退出码且不写文件；
- 修改前后均可反序列化成有效 `Config`；
- 已有 package table 不因为新增包而被整体重排；
- JSON 配置返回明确的不支持错误；
- 修改后的文档再次同步是幂等的。
- `config migrate` 将 legacy `version-mode` 转换为 `channel`，保留无关字段与注释；
- `config migrate --check` 在存在迁移项时不写文件并返回非零；
- 普通命令因 TOML 配置无法按当前契约加载时提示运行 `smif config migrate`，JSON、不存在和 I/O 错误不提示；
- 顶层 CLI 错误由 `Terminal` 而不是 logger 输出，并保留无效 TOML 的行列、源码片段和解析原因；
- 同时存在 `channel` 与 `version-mode` 时迁移拒绝写入。
- `config channel set` 与 `clear` 仅修改指定 package 的 `channel` 字段，并保留 table 的其他内容；
- `config channel --check` 在目标 channel 不匹配时不写入并返回非零；
- `config channel --all` 显式应用至每个已配置 package，重复执行无 diff。
- `config channel set --bump` 仅为实际切换 channel 的目标 package 写入一次性策略，并覆盖 preserve/patch/minor/major 的首次进入通道版本。
- `version` 仅在成功完成后清理已使用的 `channel-bump`；dry-run 和各失败路径保留它。

### 16.6 N-API 与 npm 分发测试

- Rust binding 测试锁定参数转发和退出码映射，不复制 CLI 领域行为；
- napi-rs config dry-run 能从根包的六个 target 生成对应临时平台 package；仓库不提交生成的 `npm/` 目录；
- 当前 host 通过 `napi build --platform` 构建真实 `.node` 后，使用生成 loader 执行 `smif --version`，输出版本必须与 Rust package 一致；
- `napi artifacts` 组装后，显式 artifact gate 在缺失、重复、未知文件或目标目录不匹配时失败；
- `npm pack --dry-run --ignore-scripts` 证明主包包含两个 bin 与生成 loader，临时平台包只包含对应 native binding 与必要 metadata；
- release plan 中只有 Rust CLI 与主包作为持久发布单元，版本一致且拓扑顺序为 Rust CLI、主包；
- CI 为六个 target 生成可加载的 N-API 产物，使用 napi-rs 官方 create/artifacts/prepublish 流程发布，并保持既有独立二进制 artifact 不变。

## 17. 迁移计划

### 阶段 0：特征测试

目标：锁定现有对外行为，为内部替换提供安全网。

- 为四个 resolver 建立基础 fixture；
- 覆盖现有 changeset 和 version bump 行为；
- 记录已知缺陷，避免将 bug 无意固化为新设计。

完成条件：各生态至少具有单包、monorepo 和内部依赖用例。

### 阶段 1：引入 core 与 `ReleasePlan`

目标：建立新领域模型，但暂时复用现有 resolver 获取包数据。

- 创建 `semifold-core`；
- 引入 `PackageId`、`WorkspaceGraph` 和 `ReleasePlan`；
- 实现真正的拓扑排序；
- 将 `status` 改为渲染 `ReleasePlan`。

完成条件：`status` 不再自行计算 bump，依赖环能够给出明确路径。

#### 现有 resolver 的临时转换

阶段 1 在 ecosystem adapter 完成拆分前，由 engine/应用层将现有 resolver 数据临时转换为 `PackageSnapshot`：

- `.changes/config.toml` 中的 package key 作为 `PackageId`，manifest 声明的名称保留为 `manifest_name`；
- resolver 除 `ResolvedPackage` 外，临时暴露 manifest 依赖的名称、`DependencyKind` 与原始版本约束；resolver 不负责将依赖名称解释为 `PackageId`；
- 应用层先解析所有已配置 package，再按 `(Ecosystem, manifest_name)` 建立到 `PackageId` 的映射；同一生态出现重复 manifest name 时停止转换，避免产生歧义；
- 只有在同一生态中唯一匹配到已配置 package 的依赖才转换为 `source = Manifest` 的内部 `Dependency`，未匹配项视为外部依赖，不进入 `WorkspaceGraph`；首个切片不推断跨生态依赖，后续由 `source = Config` 的显式 `depends-on` 合并；
- package 路径必须能转换为 UTF-8 相对路径；`private` 映射为 `publishable = false`；
- 转换完成后统一调用 `WorkspaceGraph` 校验并排序，不再调用各 resolver 的 `sort_packages()` 产生新架构顺序。

该转换是迁移桥接层，不作为最终 `EcosystemAdapter` 接口；切换 `status` 后再逐步删除 resolver 内的旧排序职责。

阶段 4 收敛完成后，`EcosystemAdapter` 是生态能力唯一的多态接口；旧 `Resolver` trait、
`ResolvedPackage`、`ResolvedDependency`、`create_resolver()` 以及
`Context::create_resolver()` 全部删除。discovery、workspace 加载、publish inspect 和 ecosystem
fixtures 直接消费 `PackageInspection` 与 `ManifestDependency`，不再把 adapter 输出转换回旧模型。
`ResolverType` 仅保留为配置文件中的生态选择器，由 application registry 映射到 adapter。

发布 pre-check 模板继续兼容现有 `package.name`、`package.version`、`package.path` 与
`package.private` 字段，但该视图由 application 从 `PackageInspection` 构造，不要求 ecosystem
adapter 暴露旧 `ResolvedPackage`。

### 阶段 2：引入 `config sync`

目标：将首次初始化与后续工作区同步分离，并验证生态发现接口和 TOML 增量编辑边界。

- 引入 `ConfigSyncPlan`；
- 实现 `TomlConfigEditor`；
- 让 `init` 和 `config sync` 复用 package discovery；
- 实现 added、missing、rename、move 和 conflict 分类；
- 实现 `--check` 与 `--prune`；
- 为注释保留、未知字段保留和幂等性建立 golden tests。

完成条件：工作区新增或删除包后无需重新执行 `init`，`config sync` 只产生最小 TOML diff。

### 阶段 3：计划化文件变更

目标：让 `version` 消费与 `status` 相同的 plan。

- 阶段开始时先移除旧 `version` 的 Rust 专属版本规划路径：混合生态工作区中任何受 changeset 影响的 package 都必须从同一个 `ReleasePlan` 取得发布顺序和目标版本，不能因旧的 Rust-only 当前版本 map 缺项而 panic；
- 引入 `FileEdit` 和 `VersionMap`；
- 改造 Rust 和 Node.js resolver，使其返回修改内容而不直接写入；
- 实现文件修改验证与统一应用；
- 删除 Rust/Node.js 对 `Context.version_bumps` 的依赖；
- 将 changelog 写入纳入同一执行过程。

完成条件：`status`、`version --dry-run` 和 `version` 使用相同计划；包含 Rust 与 Node.js package 的 changeset 能完成规划且不 panic。跨生态自动传播仍只在阶段 4 合并显式 `depends-on` 边后启用。

### 阶段 4：完成 ecosystem adapter 迁移

目标：移除 resolver 的全局职责。

- 迁移 Python 和 C++；
- 删除 `Resolver::sort_packages()`；
- 删除 `Resolver::publish()`；
- 删除旧 `Resolver` trait、桥接数据类型和 factory；
- 删除 adapter 对 `Context` 和 dry-run 的依赖；
- 先将现有 `semifold-resolver` 的生态能力收敛到唯一 `EcosystemAdapter` 接口；crate
  重命名为 `semifold-ecosystems` 是独立的 package 元数据迁移，不阻塞接口收敛完成。

完成条件：所有 adapter 仅执行发现、解析和变更规划。

### 阶段 5：发布引擎与外部边界

目标：以 workspace 级 `ReleaseContext` 统一 version/release PR 模板事实，以 package 级
`PublishContext` 统一 preflight、发布命令和 GitHub release。

- 定义 `ReleasePlanContext`、`ReleaseContext`、`ReleasePackageContext`、
  `PublishContext` 和按场景构造的只读模板视图；
- 从已验证 `ReleasePlan` 确定性派生 `common_version` 与 plan fingerprint；
- 将 `branches.release` 作为严格 MiniJinja 模板渲染并校验，保持现有字面量配置兼容；
- 以同一个 `ReleaseContext` 严格渲染可选的 release commit message 与 release PR title，再构造
  一次性 `ReleasePullRequestContext`，通过固定 renderer 生成稳定排序的 release PR 正文；
- 引入 `PublishPlan`；
- 抽出 `CommandRunner` 和 `RegistryClient`；
- 将重复 publish 实现替换为统一 publisher；
- 将 GitHub release 和 asset upload 移到 Forge adapter；
- 为部分发布失败提供结构化 report。

完成条件：生态 adapter 不再运行任何外部命令；version 阶段的 release branch 与 PR
消费同一个 `ReleaseContext`；publish 与 CI 的发布路径使用相同的 `PublishPlan`、
`PublishContext` 和 publisher。

### 阶段 6：拆分 `Context` 并收敛入口层

目标：清除全局可变状态和重复编排。

- 引入 `Project` 和 `ProjectLocation`；
- 移除 `Context.version_bumps`；
- 移除 `Context::create_resolver()`；
- CLI、CI 和 MCP 改用 `SemifoldService`；
- MCP 不再修改全局 current directory。

完成条件：旧 `Context` 删除，CLI 模块中不再出现领域计算或 manifest 文件操作。

## 18. 验收标准

1. `status` 和 `version` 基于同一 `ReleasePlan`。
2. 任何 ecosystem adapter 不依赖 `Context`、GitHub、HTTP 或子进程。
3. 包顺序由统一跨生态依赖图拓扑计算。
4. 依赖环返回包含完整环路的错误。
5. 所有文件修改在写入前已完整计划和验证。
6. `--dry-run` 不调用 Semifold 文件写入器或 Forge 发布客户端；registry preflight 仍执行，
   且只有配置为 `dry-run = true` 的命令可以调用命令运行器。
7. 各生态至少有单包、workspace、内部依赖和版本重写 fixture。
8. CLI、CI 和 MCP 使用同一 application service，不复制发布计算。
9. 无任何发布计算依赖 `RefCell` 或处理顺序中逐步填充的全局 map。
10. 保持现有 CLI 主要用法；Semifold 配置仅支持 TOML，所有配置字段统一为 kebab-case，不兼容 snake_case 字段。
11. `smif config sync` 能增量同步工作区包，并保留 TOML 注释、顺序、未知字段和手工配置。
12. 缺失包默认不删除，只有完整扫描成功且显式指定 `--prune` 时才允许删除。
13. `smif config sync --check` 可用于 CI 检测配置漂移。
14. 对同一工作区连续执行两次同步，第二次不产生文件修改。
15. release branch、release PR 与模板变量消费同一个 workspace 级 `ReleaseContext`，不依赖隐式主项目。
16. MiniJinja 模板严格校验未定义变量和渲染结果；workspace 级不暴露隐式 `release.version` 或 `release.tag`。
17. 固定 release branch 在配置加载与 `init` 规划时不得等于 base branch；模板渲染后的最终
    release branch 在任何版本文件写入、分支切换或强制 push 前必须再次验证与 base branch 不同。
18. 官网 Unix 与 Windows 安装脚本接受可选的具体版本参数。未传参数时必须动态查询 GitHub
    Releases，并选择最新发布、标签严格匹配 `semifold-vX.Y.Z` 的稳定 Semifold 二进制 Release；
    不得使用仓库级 `/releases/latest`，因为同一仓库中其他 package 的 Release 也可能成为 latest。
    传入 `X.Y.Z` 或 `vX.Y.Z` 时统一从 GitHub Release 标签 `semifold-vX.Y.Z` 下载对应平台资产，
    预发布版本只能通过完整版本显式安装。版本解析、Release 查询或下载失败必须终止安装，不能
    回退到固定版本，也不能将 GitHub 错误响应写为可执行文件。两个脚本还必须接受可选安装目录；
    未指定时保持 `$HOME/.local/bin`，并允许安装目录与可选版本独立组合。
19. Semifold 自身生产代码不存在可识别的主动 panic、未经验证的索引或切片路径；
    `clippy::unwrap_used`、`clippy::expect_used` 与 `clippy::indexing_slicing` 在非测试 target 上通过。
20. `npm install --global @semifold/cli` 在六个支持 target 上安装唯一匹配的 N-API 平台包，并提供与
    原生二进制行为一致的 `smif` 与 `semifold` 命令。

## 19. 开放决策

实施前还需要确定：

1. [已决定] 运行时内部依赖的新版本不满足依赖方 manifest 约束时，才自动触发依赖方 patch bump；显式 changeset 的更高 bump 优先。约束仍满足时不自动发布依赖方。
2. [已决定] 首版 Rust 仅 `[dependencies]` 参与自动版本传播；`dev-dependencies` 与 `build-dependencies` 不自动传播。
3. [已决定] 所有内部依赖类别参与拓扑排序；首版 manifest 自动传播仅支持 Rust runtime，
   development、build、peer、optional 及其他生态 manifest 依赖不自动传播，需要时使用
   `depends-on`。
4. [已决定] `PackageId` 全局唯一且运行时不隐式添加 namespace；首次 `init` 对跨生态同名 manifest
   使用 `<ecosystem>-<manifest-name>` 生成稳定 ID，必要时追加确定性数字后缀。已有配置继续使用稳定
   ID，`config sync` 不自动重命名尚未配置的跨生态冲突。
5. [已决定] post-version 命令失败时保留已写入文件和 changeset，不自动回滚；输出包含已完成文件、失败命令和未消费 changeset 的结构化恢复指引。
6. [已决定] GitHub PR 元数据查询失败时降级为无 PR 信息的 changelog，不中断 `version`，并保留可诊断的收集错误。
7. [已决定] Semifold 运行时和配置编辑只支持 TOML；JSON 配置不再维护，发现时返回明确错误。
8. 未启用 resolver 但发现对应生态 manifest 时，是提示用户启用，还是允许 `--resolver` 自动创建默认 resolver 配置。
9. [已决定] 当前不提供 `--rewrite-changesets`。rename 继续报告未消费 changeset 中的旧 PackageId，
   由用户显式确认并修改，避免同步命令隐式改写发布意图。
10. [已决定] Rust `package.version.workspace = true` 以 manifest 派生的共享
    `VersionSourceId` 表示；组内取最高 bump，要求 channel 与 `channel-bump` 一致，全部成员进入
    版本闭包，private 成员参与计算但跳过 publish，并只编辑一次
    `[workspace.package].version`。该关系不从 `ReleaseContext` 推导。
11. [已决定] 运行时 loader 保持严格判别，不默认兼容缺少 `type` 的旧 HTTP pre-check；
    `config migrate` 在严格 `Project` 加载前读取原始 TOML，为仅含 `url` 的旧结构补充
    `type = "http"`，command pre-check 仍必须显式声明 `type`。
12. [已决定] GitHub Actions workflow output 使用 6.7 节定义的 key、schema v1、环境自动启用条件、
    dry-run/失败路径语义与写入失败优先级；兼容周期和敏感字段 allowlist 同样以该节为准。
13. [已决定] ecosystem 插件首版只支持纯 Rust Boa 执行的单文件 JavaScript ESM；使用无 ambient
    文件/进程 I/O 的最小运行时、显式只读文件与受控 `fetch` capability 和固定资源限制。插件只从
    仓库内路径加载并以 SHA-256 锁定，协议使用独立 schema version，首版 package version model
    仅支持 SemVer。TypeScript SDK 与 Vite 构建检查作为独立 npm 工具提供，不进入 CLI 运行时。

### 19.1 低优先级优化：首次发布状态

manifest 中的版本不足以可靠判断 package 是否已经发布。长期可以引入 `PackageReleaseState`，由 engine 通过 registry package metadata 获取 `Unpublished` 或 `Published` 状态，再作为纯 `ReleasePlanner` 的输入。该查询与发布前“目标版本是否已存在”的 `version_exists` pre-check 是两个不同语义；网络失败、鉴权失败或无 registry 配置时不得推断为 `Unpublished`。

该能力不阻塞当前架构重构，当前明确延期，不在 pre-check 中复用或隐式实现。具体状态模型、离线
行为、registry port 和各生态实现等到出现真实首次发布规划需求后再决策。`semifold-core` 首次发布
暂时将 manifest 版本设为 `0.0.0`，使现有 minor changeset 在 alpha 通道生成 `0.1.0-alpha.0`，在
stable 通道生成 `0.1.0`。这是当前仓库的过渡措施，不定义为所有新 package 的长期通用规则。

## 20. 推荐的第一个实施切片

第一个可合并切片不应是大规模移动文件，而应是：

1. 新建 `semifold-core`；
2. 实现 `PackageId`、`PackageSnapshot`、`WorkspaceGraph` 和确定性拓扑排序；
3. 从现有 resolver 结果临时转换为 `PackageSnapshot`；
4. 实现纯 `ReleasePlanner`；
5. 仅将 `smif status` 切换到新 plan；
6. 为多层依赖、无关节点稳定顺序和依赖环建立测试。

该切片能够验证新架构的核心价值，同时不触碰真实文件写入和发布流程，回归风险最低。

## 21. 总结

Semifold 的中心抽象不应继续是同时解析、排序、写文件和发布的 `Resolver`。

新架构应当以：

- 跨生态 `WorkspaceGraph`；
- 不可变 `ReleasePlan`；
- 无副作用 `EcosystemAdapter`；
- 统一 Plan/Validate/Apply 执行模型

为核心。

这不只是为了代码整洁度，而是为了使“跨语言 monorepo 发布”成为实际存在于核心模型中的能力。

### Agent 文档入口与技能分发

产品文档继续以 `docs/content/docs/{en,zh}` 的 MDX 为唯一正文来源；Agent 阅读入口、逐页 Markdown
和 Semifold 使用 Skill 的设计委托 `documentation-redesign.md` 第 18 节。不改变 CLI、发布模型或 MCP
工具协议，不为静态文档新增 MCP 服务。仓库贡献规则与面向使用者的 Skill 分开维护。
