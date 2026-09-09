# Semifold 架构重构 TODO

> Rust 详细设计见 [Rust 架构重设计方案](docs/prd/rust-architecture-redesign.md)，文档站详细设计见
> [文档体验重构方案](docs/prd/documentation-redesign.md)。

## 总体目标

- [x] 以跨生态 `WorkspaceGraph` 和不可变 `ReleasePlan` 取代以 `Resolver` 为中心的设计
- [x] 以 workspace 级 `ReleaseContext` 统一 version/release PR 事实，并以 package 级 `PublishContext` 统一发布事实
- [x] 让 `status`、`version`、`publish` 和 CI 使用同一套应用服务
- [x] 将领域计算与文件系统、Git、HTTP、GitHub 和子进程副作用分离
- [x] 使用 `smif config sync` 增量同步 `.changes/config.toml`，不再依赖重复执行 `init`
- [x] 保持现有 CLI 主要用法稳定；配置字段统一为 kebab-case，不兼容 snake_case

## 阶段 0：建立重构安全网

### Changeset 与版本规则

- [x] 为 changeset 解析、序列化和清理建立测试
- [x] 覆盖多个 changeset 的 bump level 合并
- [x] 覆盖 semantic major、minor、patch 计算
- [x] 覆盖当前 prerelease 初次生成、递增、切换 tag 和退出 prerelease 行为
- [x] 记录当前已知缺陷，避免将错误行为固化为新规范
- [x] 为通用 `ReleaseChannel::{Stable, Named}` 建立领域测试
- [x] 覆盖缺省 `channel` 与 `channel = "stable"` 的等价性
- [x] 覆盖首次进入命名通道的 stable 基准提升、通道内序号推进、通道切换和 `Unchanged` 不推进
- [x] 覆盖 changeset changelog tag 不影响版本通道
- [x] Rust 运行时内部依赖仅在计划新版本不满足 manifest 约束时传播 patch bump

### Ecosystem fixtures

- [x] 建立 Rust fixture：单包、workspace、内部依赖、workspace dependency、private package
- [x] 建立 Node.js fixture：单包、npm workspace、pnpm workspace、dependencies、peerDependencies
- [x] 建立 Python fixture：PEP 621、Poetry、Hatch、setup.cfg、常见 monorepo 目录
- [x] 建立 C++ fixture：CMakeLists.txt、vcpkg.json
- [x] C++ 支持根 CMake workspace 的直接 `add_subdirectory` 成员发现与 `target_link_libraries` 内部依赖排序
- [x] 为 manifest 解析结果建立 snapshot/golden tests
- [x] 为版本重写后的完整 manifest 建立 golden tests
- [x] 确保与版本无关的字段和格式不会被破坏

### 阶段完成条件

- [x] 四个生态均至少覆盖单包、工作区发现、内部依赖和版本重写
- [x] 现有 CLI 核心行为具备可回归的基线

## 阶段 1：引入 `semifold-core`

### Crate 与基础类型

- [x] 创建 `crates/semifold-core`
- [x] 定义 `PackageId` newtype
- [x] 定义 `Ecosystem`
- [x] 定义 `PackageSnapshot`
- [x] 定义 `Dependency` 和 `DependencyKind`
- [x] 定义 `ChangesetId`、`ReleaseReason` 和 `PlanWarning`
- [x] 确保 core 不依赖 `git2`、`reqwest`、`octocrab`、`clap` 或 `inquire`

### Workspace graph

- [x] 实现 `WorkspaceGraph`
- [x] 验证重复 `PackageId`
- [x] 验证未知内部依赖
- [x] 实现确定性拓扑排序
- [x] 对无依赖节点使用稳定排序
- [x] 检测依赖环并返回完整环路
- [x] 为多层依赖、菱形依赖、无关节点和依赖环建立单元测试

### Release planner

- [x] 定义不可变、可序列化的 `ReleasePlan`
- [x] 定义 `PackageRelease` 和完整 `VersionMap`
- [x] 实现纯 `ReleasePlanner`
- [x] 合并多个 changeset 的发布原因和 bump level
- [x] 在写文件前计算所有包的下一版本
- [x] 明确 stable / 命名版本通道与依赖传播规则
- [x] 临时将现有 resolver 结果转换为 `PackageSnapshot`

### 接入 `status`

- [x] 将 `smif status` 切换为渲染 `ReleasePlan`
- [x] 保持现有终端输出语义
- [x] CI PR comment 使用同一 `ReleasePlan`
- [x] 删除 `status` 内重复的版本计算

### 阶段完成条件

- [x] `status` 不再自行计算 bump
- [x] 相同输入始终生成相同 `ReleasePlan`
- [x] 依赖环错误包含可读的完整路径

## 阶段 2：实现 `smif config sync`

### CLI

- [x] `config channel set` 支持一次性 `--bump preserve|patch|minor|major`，并由成功的 `version` 消费
- [x] 新增 `smif config migrate`
- [x] 新增 `smif config migrate --check`
- [x] 新增 `smif config channel set <channel> --package <PackageId>` 与 `--all`
- [x] 新增 `smif config channel clear --package <PackageId>` 与 `--all`
- [x] 支持 `smif config channel ... --check`
- [x] `config channel set` 在 Node.js package 的 `npm publish` 缺少显式 `--tag` 时提示目标 package 与 channel，不自动改写命令
- [x] `config migrate` 在严格 `Project` 加载前直接从原始 TOML 规划、验证并原子应用迁移
- [x] 严格加载 TOML 配置失败时追加本地化的 `smif config migrate` 恢复建议，并排除迁移无法处理的错误
- [x] 顶层命令错误改由 `Terminal` 输出，并为配置加载错误保留完整底层 TOML 诊断
- [x] 新增 `smif config sync`
- [x] 新增 `smif config sync --check`
- [x] 新增 `smif config sync --prune`
- [x] 支持重复指定 `--resolver <type>`
- [x] 明确全局 `--dry-run` 与 `--check` 的不同退出语义
- [x] JSON 配置返回 `UnsupportedConfigFormat`

### 同步计划

- [x] 定义 `ConfigSyncPlan`
- [x] 分类 added、missing、renamed、moved 和 conflicts
- [x] 实现 package path 规范化
- [x] 优先使用 resolver + path 匹配重命名
- [x] 使用 package identity 匹配路径移动
- [x] 多义匹配时停止写入并报告冲突
- [x] Resolver 变化时要求显式处理，不自动覆盖
- [x] Rename 时检查现有 changeset 对旧包名的引用并警告

### TOML 增量编辑

- [x] 将 legacy `version-mode` 增量迁移为 `channel`，并保留无关字段和注释
- [x] `channel` 与 `version-mode` 同时存在时停止并报告冲突
- [x] 实现 `TomlConfigEditor`
- [x] 使用 `toml_edit::DocumentMut` 读取和修改原始文档
- [x] 修改前将文档反序列化为强类型 `Config` 并验证
- [x] 只更新 `[packages]` 下需要变化的 table
- [x] 保留 `[branches]`、`[release]` 和其他发布策略配置
- [x] 保留注释、空行、字段顺序和未知字段
- [x] 保留 `assets`、`channel` 和 `depends-on` 等手工字段
- [x] 在配置模型中以 `ReleaseChannel::{Stable, Named}` 取代 `VersionMode`（仅保留 legacy 解析兼容）
- [x] 读取时接受缺省 `channel` 和 `channel = "stable"`，并将二者解析为 stable
- [x] 新增 stable package 时省略 `channel`，无关同步时保留用户显式的 `channel = "stable"`
- [x] 将 `channel` 与 changeset / changelog 的 tag 完全分离
- [x] 新增 package 时只写入最小默认配置
- [x] 新增条目采用确定性顺序，但不重排现有条目
- [x] 默认只报告缺失 package
- [x] 仅在完整扫描成功且指定 `--prune` 时删除 package
- [x] 修改后再次反序列化并验证 `Config`
- [x] 使用临时文件和 rename 原子写回
- [x] 内容无变化时不写文件

### 与 `init` 复用

- [x] 抽取 resolver registry
- [x] 抽取统一 package discovery 服务
- [x] 抽取 package path 规范化
- [x] 抽取默认 `PackageConfig` 生成
- [x] 让 `init` 和 `config sync` 使用相同发现逻辑
- [x] `init` 为跨生态重复的默认 `PackageId` 建议添加 ecosystem 前缀，并以确定性数字后缀处理二次冲突
- [x] 将 `init --force` 从日常工作区同步路径中移除（仅保留为显式重新初始化）

### 测试

- [x] `config migrate --check` 检测到旧字段时返回非零且不写文件
- [x] 连续执行两次 `config migrate` 时第二次无 diff
- [x] 配置加载迁移建议覆盖可迁移旧契约，并确认 JSON、不存在和 I/O 错误不会得到误导提示
- [x] malformed TOML 的 CLI 回归测试覆盖错误标签、行列源码诊断和迁移建议
- [x] channel set、clear 与 `--all` 保留目标 table 的其他字段，并且重复执行无 diff
- [x] channel `--check` 检测到状态不匹配时返回非零且不写文件
- [x] 新增 package 只产生最小 TOML diff
- [x] 缺失 package 默认保留
- [x] `--prune` 显式删除缺失 package
- [x] 扫描失败、部分 resolver 或匹配歧义时禁止 prune
- [x] Rename 和 move 保留原 table 的注释与自定义字段
- [x] `--check` 检测到漂移时返回非零退出码且不写文件
- [x] 连续执行两次同步时第二次无 diff

### 阶段完成条件

- [x] 工作区增删包后无需重新执行 `init`
- [x] `config sync` 只产生最小、可审查的 TOML diff
- [x] CI 可以使用 `config sync --check` 检测配置漂移
- [x] 阶段完成前清除为分阶段接线临时添加的全部 `#[allow(dead_code)]`，并以无豁免的 Clippy 验证

## 阶段 3：统一版本修改的 Plan/Validate/Apply

### 文件修改模型

- [x] 定义 `FileEdit`、`FileHash` 和 `EditSource`
- [x] 在计划阶段生成所有 manifest 和 changelog 修改
- [x] 检查多个 edit 是否修改同一文件
- [x] 使用 expected hash 检测规划后的并发修改
- [x] 实现临时文件写入和原子替换
- [x] 所有文件成功写入后再删除 changeset
- [x] post-version 失败时保留已写入文件和 changeset，并在 `ApplyReport` 中返回恢复事实

### Rust 与 Node.js

- [x] 将 Rust resolver 改为根据完整 `VersionMap` 生成 `FileEdit`
- [x] 正确更新 Rust dependencies、dev-dependencies、build-dependencies 和 workspace dependencies
- [x] 将 Node.js resolver 改为生成 `FileEdit`
- [x] 正确更新 dependencies、devDependencies、peerDependencies 和 optionalDependencies
- [x] 移除 Rust/Node.js 对 `Context.version_bumps` 的依赖
- [x] Resolver 不再直接写版本文件或处理 `version` dry-run；publish 命令与 publish dry-run 在阶段 4 随 `Resolver::publish()` 移除

### Python 与 C++

- [x] Python 只规划 `pyproject.toml`、`setup.cfg` 和 Python 源码版本文件，不写入 `Cargo.toml`
- [x] Python 动态版本可继续读取 `Cargo.toml`，但跨生态派生关系留待显式 `version-source`
- [x] C++ 将 `CMakeLists.txt` 与可选 `vcpkg.json` 表示为 `FileEdit`
- [x] Python/C++ 不再通过 `Resolver::bump()` 直接写入版本文件

### Changelog

- [x] 分离 Git/Forge 元数据收集与 Markdown 格式化
- [x] 定义不可变 `ChangelogContext`
- [x] 让 changelog 格式化成为纯函数
- [x] 按空行划分 changeset summary 段落，将续段缩进在同一 Markdown 列表项内，并将段内编辑换行规范化为空格
- [x] 为依赖传播自动加入发布闭包的 package 生成 `Dependencies` changelog 条目
- [x] 将 Rust/Node changelog 修改表示为 `FileEdit`
- [x] GitHub PR 元数据查询失败时降级为无 PR 信息的 changelog，不中断 `version`
- [x] 新增 workspace 级 `[changelog]`、`template` 和 `changeset-template` 配置，缺省时使用内置默认模板
- [x] 扩展 `CommitContext` 的 `short_sha`、commit author 以及 `ChangesetContext.summary_paragraphs`
- [x] 使用 strict MiniJinja 两阶段渲染 changeset 和 package release block，并保持默认输出兼容
- [x] 为整体模板暴露原始 changesets、依赖更新以及带预渲染 content 的稳定 section 视图
- [x] 使用 Semifold release marker 插入和读取任意格式 block，并兼容解析旧 `##` changelog
- [x] 在 `prepare_release` 和 dry-run 中完成模板编译、渲染与结构校验，失败时保持零文件副作用
- [x] 为配置、模板作用域、空白、多段 summary、metadata、marker、兼容解析和 CLI 路径建立回归测试
- [x] `smif init` 显式写入与运行时 fallback 同源的默认 `[changelog]` 模板，便于用户直接修改

### 接入 `version`

- [x] 移除旧 Rust 专属版本规划路径；混合 Rust + Node.js changeset 必须与 `status` 使用同一 `ReleasePlan` 且不 panic
- [x] `smif version` 消费与 `status` 相同的 `ReleasePlan`
- [x] Rust 与 Node.js manifest 由 `ReleasePlan.file_edits` 统一规划、校验并原子应用
- [x] `smif version --dry-run` 不调用写入器，仅运行显式配置 `dry-run = true` 的
  post-version 命令，其余命令跳过
- [x] 删除 `version` 内逐包填充可变版本 map 的逻辑
- [x] 返回结构化 `ApplyReport`

### 阶段完成条件

- [x] `status`、`version --dry-run` 与 `version` 使用相同计划
- [x] 任一验证失败时工作区保持不变
- [x] 文件修改不再依赖包处理顺序

## 阶段 4：将 Resolver 收敛为 Ecosystem Adapter

### 新接口

- [x] 定义 `EcosystemAdapter`
- [x] 实现 `discover()`
- [x] 实现 `inspect()`
- [x] 实现 `plan_edits()`
- [x] Adapter 接收完整 `VersionMap`
- [x] 由各 ecosystem adapter 验证并编码命名通道版本；支持 Python PEP 440 post-release 格式

### 迁移

- [x] 迁移 Rust adapter（discovery、workspace inspection 与 version edit planning 已接入；旧 publish 桥接由统一发布引擎移除）
- [x] 迁移 Node.js adapter（discovery、workspace inspection 与 version edit planning 已接入；旧 publish 桥接由统一发布引擎移除）
- [x] 迁移 Python adapter（discovery、workspace inspection 与 version edit planning 已接入；动态版本只读 Cargo.toml，旧 publish 桥接由统一发布引擎移除）
- [x] 迁移 C++ adapter（递归 discovery、workspace inspection 与 version edit planning 已接入；旧 publish 桥接由统一发布引擎移除）
- [x] 删除 `Resolver::sort_packages()`
- [x] 删除 `Resolver::publish()`（发布命令由 application 层统一执行）
- [x] 删除 adapter 对 `Context` 的依赖
- [x] 删除 adapter 内的 dry-run 分支
- [x] 将 `semifold-resolver` 的生态能力收敛到唯一 `EcosystemAdapter` 接口（crate 重命名单独处理）

### 跨生态依赖

- [x] 在配置中增加可选 `depends-on`
- [x] 将 manifest 内部依赖与显式跨生态依赖合并到 `WorkspaceGraph`
- [x] 定义相同包名跨生态冲突策略
- [x] 定义 dev、peer、optional 和 build dependency 的排序与传播策略

### 阶段完成条件

- [x] Adapter 只负责发现、解析和变更规划
- [x] 所有包顺序由统一 `WorkspaceGraph` 计算
- [x] 跨生态依赖参与同一拓扑排序

## 阶段 5：统一发布引擎

### 配置字段规范化

- [x] 将 TOML 配置字段统一为 kebab-case，更新仓库配置、初始化模板、示例和 fixture；
  不为 snake_case 字段提供 serde alias
- [x] 扩展 `config migrate`，原位重命名已知 snake_case 字段；新旧字段同时存在时报告冲突

### Workspace release context

- [x] 定义可序列化的 `ReleasePlanContext`、`ReleaseContext`、`RepositoryContext` 和 `CiContext`
- [x] `ReleasePlanContext.changesets` 直接使用稳定排序的 `Vec<ChangesetId>`，不复用 changelog `ChangesetContext`
- [x] `ReleasePlanContext.packages` 只包含实际发布 package，并按 `PackageId` 稳定序列化
- [x] 仅在所有实际发布 package 的 `next_version` 相同时生成 `common_version`
- [x] 以规范化 package 版本与 changeset ID 生成确定性 SHA-256 plan fingerprint，对外使用前 12 位小写十六进制
- [x] 首版不引入没有明确消费者的 `ProjectContext`，也不向模板暴露项目绝对路径或完整配置
- [x] 为单包、多包同版本、多包不同版本、空计划和输入顺序无关性建立上下文测试
- [x] 实现已确认的 Rust 共享 `VersionSource`：组内最高 bump、channel / `channel-bump` 一致性、全成员版本闭包、private publish skip 和 `[workspace.package].version` 单点编辑
- [x] 为 workspace 继承版本建立 discovery、status 和 version 不 panic 回归测试

### Publish plan

- [x] 为发布 PR 正文增加 65,536 字节预算、超限版本摘要与双语提示，验证边界并同步文档

- [x] 定义可从当前 workspace 重建的 `PublishContext`、`PublishPlan` 和 `PackagePublish`
- [x] publish 不依赖或持久化 version 阶段的 `ReleaseContext`，也不从已消费 changeset 反推发布集合
- [x] 依据 `ReleasePlan` 构造唯一 workspace `ReleaseContext`
- [x] 从同一个 `ReleaseContext` 渲染 release branch，并构造一次性
  `ReleasePullRequestContext` 供固定兼容 renderer 生成 release PR；不隐式选择主 package，
  不将 changelog 写回 `ReleaseContext`
- [x] 明确 package-level Git tag 与 workspace release branch / PR 的区别
- [x] 将 preflight、commands 和 assets 纳入计划
- [x] asset 在计划中仅保留已校验声明，package 命令成功后才展开 glob 并生成稳定
  `ReleaseAsset`，不得遗漏命令生成的产物
- [x] 基于 `WorkspaceGraph` 生成确定性发布顺序
- [x] 为 private package 的 registry 发布和已发布版本提供显式 skip reason
- [x] 缺失 `CHANGELOG.md` 的 package 以 `MissingChangelog` 跳过全部发布流程
- [x] 增加 package 级 `github-release` 三态策略；保持 public/private 缺省行为，并允许显式控制
  private package 的 GitHub Release 与 asset upload
- [x] 增加 package 级可选 `publish` 覆盖；缺省沿用 manifest/plugin 发布标识，显式值覆盖
  registry preflight 与发布命令资格，并由 config sync 保留

### 外部能力

- [x] 抽取 `CommandRunner`
- [x] 抽取 `RegistryClient`
- [x] 抽取 `ForgeClient`
- [x] 将 GitHub release 创建移到 Forge adapter
- [x] 将 asset upload 移到 Forge adapter
- [x] 删除四个生态中重复的 publish 命令执行代码
- [x] 将 `CommandSpec` 的 `dry-run` 作为全局 dry-run 下执行该命令的显式许可；registry
  preflight 保持只读执行，Forge release 与 asset upload 必须跳过

### 执行与报告

- [x] 在执行发布前完成所有可执行的 preflight
- [x] HTTP pre-check 运行时注入可覆盖的默认 User-Agent，并支持显式 retry 延迟与有界响应诊断
- [x] 定义 `ReleasePackageContext`、`PublishContext`、`ChangelogContext` 和按场景构造的只读模板视图
- [x] 定义 changelog 专用 `ChangesetContext`、`PackageChangesetContext` 和 `DependencyUpdateContext`
- [x] `ChangelogContext` 按 package 聚合 changesets 和依赖更新；tag 在收集层解析为 section，不进入 `ChangesetContext`
- [x] 分支模板只暴露 `release.*`
- [x] changelog 与 version 包级模板暴露 `release.*`、`package.*`；publish 模板只暴露可从当前 package 重建的 `package.*`
- [x] 使用 MiniJinja 严格未定义变量模式
- [x] changelog 整体与单条模板使用 MiniJinja 严格未定义变量模式，并返回带 package/changeset 的结构化错误
- [x] 在渲染后校验 branch ref、Git tag 和命令参数
- [x] workspace 模板中不暴露 `release.tag` 或 `release.version`；`common_version = None` 时引用它必须返回配置错误
- [x] 以 `ReleaseContext` 渲染 `branches.release`，并保持无模板语法的现有字面量配置
- [x] 支持 dry-run 发布计划
- [x] 返回结构化 `PublishReport`
- [x] 报告 succeeded、skipped、failed 和 not-started package
- [x] 明确部分发布失败后的退出码和恢复指引
- [x] registry 版本已存在时仅跳过 registry 命令，继续首次 GitHub Release；已有 Release 不恢复 asset，并组合展示两类事实

### 阶段完成条件

- [x] Ecosystem adapter 不再执行任何外部命令
- [x] `publish` 与 CI 使用相同 `PublishPlan` 和 publisher

## 阶段 6：拆分 `Context` 并收敛入口层

### 项目加载

- [x] 从磁盘加载 changeset 时严格校验 front matter 分隔符、非空 package 集合和非空 summary
- [x] 定义字段完整的 `Project`
- [x] 为 `init` 定义独立 `ProjectLocation`
- [x] 定义结构化 `ProjectLoadError`
- [x] 以 `ProjectLoadError::NonUtf8Path` 显式拒绝无法无损表示的项目路径
- [x] 不再使用 `.ok()` 吞掉配置、路径或 Git 错误
- [x] GitHub 环境、Git 句柄和 dry-run 不进入 `Project`
- [x] 将发布事实建模为不可变 `ReleaseContext`，而不是恢复万能 `Context`
- [x] 将 version 包级事实建模为 `ReleasePackageContext`，将发布事实建模为独立 `PublishContext`
- [x] 将 changelog 所需事实建模为 `ChangelogContext`
- [x] 确保 context 不持有 Git、HTTP、文件系统、resolver 或可变缓存

### 应用服务

- [x] 创建 `semifold-engine`
- [x] 实现 `SemifoldService::plan_init()` / `apply_init()`，并让 `init` 与 `config sync` 复用 package discovery
- [x] 实现 CLI/MCP 共用的 `SemifoldService::create_changeset()`
- [x] 实现 `SemifoldService::plan_config_sync()`
- [x] 实现 `SemifoldService::apply_config_sync()`
- [x] 实现 `SemifoldService::plan_release()`
- [x] 实现 `SemifoldService::prepare_release()`，将纯领域 `ReleasePlan` 准备为完整 `ReleaseApplyPlan`
- [x] 实现 `SemifoldService::apply_release()`
- [x] 实现接收显式 `PublishOptions` 并生成完整 Forge/asset 执行事实的 `SemifoldService::plan_publish()`
- [x] 实现只消费 `PublishPlan` 与 `ExecutionMode` 的 `SemifoldService::publish()`

### CLI、CI 与 MCP

- [x] CLI 只负责参数解析、交互和结果渲染
- [x] CI 通过 `SemifoldService` 编排 release branch、commit、push 和 PR
- [x] MCP 使用与 CLI 相同的应用服务
- [x] MCP 不再为每次调用重新构建全局 `Context`
- [x] MCP 不再使用 `set_current_dir()` 修改进程全局状态
- [x] MCP transport 在严格 `Project` 加载前启动，并由固定范围 `ProjectLocator` 为每次工具调用刷新项目
- [x] MCP 只声明已实现的 tools capability，并返回实际 server version 与结构化 output/error
- [x] MCP changeset 支持带 revision 的 get/create/update/delete，create 幂等且不提供 rename
- [x] MCP 全局 `--dry-run` 验证但不应用 changeset CRUD 写操作
- [x] MCP 覆盖无效项目、无效输入、冲突、revision 失配、dry-run、panic 隔离和调用后继续服务测试
- [x] 移除旧 `Context.version_bumps`
- [x] 移除旧 `Context::create_resolver()`
- [x] 删除旧 `Context`

### 错误边界

- [x] Core 使用结构化 `DomainError`
- [x] Ecosystem adapter 使用结构化 `AdapterError`
- [x] Engine 使用结构化 `AppError`
- [x] 仅在 CLI 最外层使用 `anyhow` 补充上下文
- [x] 本地化和终端着色不进入 core 或 engine
- [x] 消除生产代码中的 `unwrap`、`expect`、主动 panic、未经验证的索引与切片，并在非测试
  target 启用对应 Clippy 验收

### 阶段完成条件

- [x] CLI 模块中不再包含版本计算或 manifest 文件操作
- [x] CLI、CI 和 MCP 不复制业务编排
- [x] 发布计算中不存在 `RefCell` 或隐式全局可变 map

## 阶段 7：向 CI/CD 暴露版本化工作流输出

### 输出契约

- [x] 确定 `version` / `publish` 的 GitHub Actions output key、启用方式和 schema 兼容周期
- [x] 定义带 `schema-version` 的 version workflow DTO，覆盖 plan fingerprint、release branch 和
  实际发布 package 的稳定版本事实
- [x] 定义带 `schema-version` 的 publish workflow DTO，覆盖 package 发布状态和部分失败恢复事实
- [x] 确定 dry-run、publish 失败及 output 写入失败的优先级和退出语义
- [x] 建立敏感字段 allowlist，禁止输出 header、环境变量、token、命令配置和 author email

### 应用与 GitHub Actions 边界

- [x] 从 version 使用的同一个 `ReleaseContext` 派生 workflow output，不在 changeset 消费后重新推断
- [x] 从 `PublishPlan` 与 `PublishReport` 派生 publish workflow output，并保留 succeeded、skipped、
  failed 和 not-started 状态
- [x] 抽取 workflow output writer port，在 CLI 最外层实现 GitHub Actions 安全多行输出格式
- [x] 非 GitHub Actions 环境不写额外文件，也不改变现有终端输出
- [x] 让 `smif version` 与 `smif publish` 通过同一个 application output 契约提供后续 step/job 数据
- [x] 让 `smif ci` 委托 version/publish 的既有 output 路径，不复制 engine 编排或 DTO 写入
- [x] 为内置模板和项目 workflow 的 Semifold step 设置稳定 ID，并映射 `version` / `publish` job outputs

### 阶段完成条件

- [x] 后续 GitHub Actions job 无需重新读取 changeset，即可取得 version 或 publish 阶段的确定性事实
- [x] publish 部分失败时仍能取得完整结构化恢复状态
- [x] workflow output schema 具有兼容性测试，且敏感字段不会进入输出

## 阶段 8：现代化 CLI 终端反馈

### 展示基础层

- [x] 定义 stdout/stderr、TTY/CI、`NO_COLOR`、Unicode 与 dry-run 输出契约
- [x] 建立 CLI-only `Terminal`、presentation model 与 `ProgressReporter` 边界
- [x] 使用 `indicatif` 实现动态进度，并为非 TTY 提供稳定 plain 降级
- [x] 让 logger 与动态进度共享输出协调机制，禁止日志破坏动态区域
- [x] 外部命令继承终端输出时暂停动态进度，避免 spinner 覆盖子进程输出
- [x] 键值事实与表格统一按 Unicode 显示宽度对齐
- [x] 通过 release-apply callback 展示 post-version package 范围与逐命令状态，并依据 stdio 策略选择动态或静态反馈
- [x] 移除 debug 中的完整配置、GitHub event 和其他潜在敏感信息

### 核心命令

- [x] `status` 展示计划 fingerprint、bump、原因和明确完成摘要
- [x] 优化 `status` GitHub PR comment，并为空计划解释合入后的未发布版本发布行为
- [x] 让 `status` comment 的计划 commit SHA 使用 GitHub 可自动链接的裸文本
- [x] 让 `status` comment 写入失败 warning 展示操作、GitHub API 状态、message、文档链接与 403 权限提示，同时保持非致命
- [x] 让多事实 comment warning 使用带语义标签的缩进层级，单事实客户端错误保持单行
- [x] 在中英文首次发布指南记录 GitHub Actions workflow 的 read/write 与 PR 创建权限前置设置
- [x] 通过 GitHub PR Files API 检测并展示当前分支引入或变更的 changeset
- [x] `version` 展示 plan/prepare/validate/apply/post-version/changeset 消费阶段与最终版本表
- [x] `publish` 展示 preflight、命令、Forge、asset 与 succeeded/skipped/failed/not-started 摘要
- [x] `publish` 结果表在列宽计算后按 package、版本与四态结果应用语义颜色
- [x] dry-run 在开头和结尾明确标识未应用的副作用
- [x] 部分失败展示已完成、失败、未开始事实和可执行恢复建议

### 其余入口与测试

- [x] 减少中文 CLI 文案中的分号，优先改用短句、逗号或句号
- [x] 让全局 `--dry-run` 覆盖 `init`、`commit` 与 `status --comment`，统一禁止文件和 PR comment 写入
- [x] 统一 config、init、commit 和 CI 的标题、warning 与最终成功反馈
- [x] 建立“交互仅为参数缺省回退”的 CLI 约定，并让 init/commit 可通过完整参数在关闭 stdin 时运行
- [x] 将 init 的 workflow 选择参数统一为 `--github-actions` / `--no-github-actions`，并移除旧名称
- [x] 让 `commit -m` 可重复传入，并将每个值作为独立 changeset summary 段落写入和渲染
- [x] 所有新增或修改的用户文案同步 en/zh locale，key 集合保持一致
- [x] 建立 TTY/非 TTY、成功、skip、dry-run、部分失败、宽字符和敏感信息测试

### 阶段完成条件

- [x] 人类可读业务输出不再直接依赖散落的 `println!` 与 `log::*`
- [x] 非 TTY/CI 输出稳定且不含光标控制序列，TTY 动态进度不会被日志破坏
- [x] 每个用户命令成功时有明确摘要，失败时包含阶段和适用的恢复建议

## 阶段 9：可扩展 ecosystem 插件

### 协议与身份

- [x] 决定首版使用 JavaScript、Lua 或同时支持二者，并确定沙箱、capability、资源限制与超时策略
- [x] 决定插件分发、版本锁定、协议兼容周期，以及首版版本模型是否限定为 SemVer
- [x] 将闭集 `Ecosystem` / `ResolverType` 演进为稳定 `EcosystemId`，同时保留内置生态固定 ID
  - [x] 领域 package、plan/context、config sync 与 adapter 使用开放 `EcosystemId`
  - [x] package/resolver 配置与 registry 从内置 `ResolverType` 开放到动态 ID
- [x] 定义带 schema version 的插件元数据及 discover、inspect、plan-edits 序列化协议
- [x] 定义插件结构化诊断，携带插件、操作、package 和相关路径

### Host 与 adapter 集成

- [x] 实现 Boa 脚本运行时 host 和稳定插件注册表，加载结果不得依赖发现顺序
  - [x] 嵌入 Boa，以拒绝 import 的 loader 执行单文件 ESM，并支持同步或异步默认入口
  - [x] 注入受项目边界与预算约束的文件 capability
  - [x] 以稳定 ecosystem ID、仓库内相对路径和可选 SHA-256 内容锁加载插件注册表
  - [x] 注入默认拒绝、按 HTTPS origin 授权且可替换 backend 的 `fetch` capability
- [x] 将插件协议适配到现有 `EcosystemAdapter` 边界，不允许插件恢复全局 resolver 职责
- [x] 禁止插件直接写文件、运行发布命令、取得 registry/Forge 宿主凭据或创建 Forge release
- [x] 对插件返回的 package、依赖和候选 `FileEdit` 复用 host 的路径、hash、冲突和依赖图校验
- [x] 让 discovery、workspace load、`config sync` 和 version 规划支持动态 ecosystem ID

### TypeScript SDK 与构建工具

- [x] 新增独立发布的 `@semifold/plugin-sdk` ESM package，并接入 Bun workspace、Semifold release plan 与 OIDC-only npm publish workflow
- [x] 导出 schema v1 的 kebab-case wire types、metadata/entrypoint/diagnostic/response 构造辅助函数
- [x] 精确声明 `PluginHostV1` 与 Boa 实际支持的 `fetch`、`URL` 子集，不引入 DOM 或 Node.js ambient API
- [x] 以无 DOM/Node ambient 的类型 fixture 和运行时 JSON 测试锁定 SDK/Rust 协议边界
- [x] 从 Rust 插件协议自动生成 TypeScript wire types
  - [x] 使用 `ts-rs` 投影 serde 字段、tag、optional 规则以及 schema/operation 字面量
  - [x] 从生成 union 派生现有 operation-specific 类型，并在公共出口递归施加只读约束
  - [x] 提供确定性生成与 CI drift 检查，同时保留跨语言 JSON fixture
  - [x] 生成与 drift 检查统一经过固定版本 Biome，并将 SDK 的 Biome format/lint 接入只读检查与 CI
- [x] 将仓库 JavaScript 工具链、锁文件和 CI/CD 从 pnpm 迁移到显式 `canary` 通道的 Bun
- [ ] 实现配套 Vite 插件的单文件 ESM bundle 与不支持能力检查

### 阶段完成条件

- [x] 至少一个仓库外脚本插件完成单包、workspace、内部依赖和版本修改 fixture
- [x] 同一插件输入与 capability 响应记录重复运行产生稳定结果，越界路径、非法 edit、预算超限和协议不兼容均返回结构化错误
- [x] ecosystem 插件不改变 publish/pre-check/Forge 的既有应用层职责边界

## 阶段 8：N-API 与 npm CLI 分发

### Rust 与 Node-API 边界

- [x] 新增薄 `semifold-napi` crate，以 N-API 8 导出同步 CLI 参数入口和退出码
- [x] 让独立二进制与 N-API binding 复用显式参数驱动的 CLI runner
- [x] 保持 cwd、环境变量、stdio、TTY、MCP transport 与本地化全部由既有 Rust CLI 处理

### npm package 与平台产物

- [x] 新增公开 `@semifold/cli` wrapper，提供 `smif` 与 `semifold` 两个 bin
- [x] 在根包配置 napi-rs v3 的 binary name 与 macOS、Linux glibc、Windows x64/arm64 targets
- [x] 使用 napi-rs 生成 loader，并在 CI 临时生成、组装和发布 optional platform packages
- [x] 只将主包接入 workspace、跨生态依赖图和 changeset，不提交或配置临时平台包

### CI、测试与文档

- [x] 在六 target 构建中同时生成独立二进制与 `.node` binding，并在发布前运行 `napi create-npm-dirs` 与 `napi artifacts`
- [x] 覆盖 napi config、真实 generated-loader smoke、pack 内容和缺失 artifact 失败路径
- [x] 将中英文安装文档中的 npm 命令切换为已验证的 `@semifold/cli`

## 最终验收

- [x] `cargo fmt --all --check` 通过
- [x] `cargo clippy --workspace --all-targets --all-features` 通过
- [x] `cargo test --workspace --all-features` 通过
- [x] `status` 与 `version` 的计划结果完全一致
- [x] 跨生态依赖图支持确定性拓扑排序和环检测
- [x] 所有文件修改在写入前完成计划和验证
- [x] `--dry-run` 不应用 Semifold 文件修改、不创建 Forge release 或上传 asset；只执行 registry
  preflight 与显式配置 `dry-run = true` 的命令，并在报告中区分实际执行和跳过
- [x] `config sync` 保留 TOML 注释、顺序、未知字段和手工配置
- [x] `config sync --check` 可稳定用于 CI
- [x] 连续执行配置同步和版本规划均具有幂等性
- [x] 非测试 target 通过 `clippy::unwrap_used`、`clippy::expect_used` 和
  `clippy::indexing_slicing`，生产代码不存在可识别的 panic 路径
- [x] release branch / release PR 消费同一 workspace `ReleaseContext`，并支持固定分支与显式 plan/package 模板
- [x] 配置加载与 `init` 拒绝和 base 相同的固定 release branch；模板渲染后在版本文件写入、
  分支切换或强制 push 前再次拒绝与 base 相同的最终 release branch
- [x] `--release-branch` help 与中英文配置、初始化和 CI 文档明确 release branch 必须独立于
  base branch，且会由自动化强制维护
- [x] 模板在严格模式下渲染，且 workspace 发布不会隐式选择 package version 或 tag
- [x] changelog 默认模板保持现有输出，自定义模板可控制 release block 和单条 changeset 的渲染格式
- [x] publish 能读取 marker 包围的自定义 changelog，并对无 marker 的旧格式保持兼容
- [x] 现有 CLI 主要用法保持稳定；所有 TOML 配置字段使用 kebab-case，snake_case 不兼容
- [x] 官网 Unix 与 Windows 安装脚本动态解析最新稳定的 `semifold-vX.Y.Z` Release，兼容
  `X.Y.Z` / `vX.Y.Z` 显式版本，且不依赖仓库级 `/releases/latest` 或固定 fallback 版本
- [x] 官网 Unix 与 Windows 安装脚本支持独立指定安装目录，并保持默认目录兼容
- [x] `version` 与 `publish` 可以向 GitHub Actions 后续流程提供版本化、无敏感信息的结构化输出
- [x] 特定领域项目可以通过受控脚本插件接入 ecosystem discovery、inspection 与 edit planning
- [ ] `npm install --global @semifold/cli` 在六个支持平台上提供与原生二进制一致的命令行为

## 实施前需要确认的决策

- [x] 运行时内部依赖仅在计划新版本不满足依赖方 manifest 约束时触发 patch bump；显式 changeset 的更高 bump 优先，约束仍满足时不自动发布依赖方
- [x] 首版 Rust 仅 `[dependencies]` 参与自动版本传播；`dev-dependencies` 与 `build-dependencies` 不自动传播
- [x] 所有内部依赖类别参与排序；dev、build、peer、optional 不自动传播，需要时使用 `depends-on`
- [x] `PackageId` 运行时不隐式添加 namespace；首次 `init` 对跨生态同名使用 ecosystem 前缀和确定性后缀，已有配置继续使用稳定 ID
- [x] Post-version 命令失败时保留已写入文件和 changeset，不自动回滚，并返回结构化恢复指引
- [x] GitHub PR 元数据查询失败时降级为无 PR 信息的 changelog，不中断 `version`
- [x] 移除 Semifold JSON 配置加载与保存；发现 `config.json` 时返回明确的不支持错误
- [x] `config sync` 遇到未启用 resolver 时返回 `ResolverNotEnabled`
- [x] 当前不提供 `--rewrite-changesets`；rename 只报告 changeset 中的旧 PackageId，由用户显式修改
- [x] Rust workspace 继承版本的共享版本来源、bump 合并、channel 与发布闭包规则

## 低优先级优化

- [x] `PackageReleaseState` 与 registry package metadata 查询明确延期，不复用目标版本 pre-check
- [x] 将 publish pre-check 改为带 `type` 的 `http` / `command` 强类型配置
- [x] HTTP pre-check 仅将 200 视为存在、404 视为不存在，其他状态均失败
- [x] command pre-check 实现固定 JSON Lines stdin/stdout 契约，并在 dry-run 中照常执行
- [x] 旧 HTTP pre-check 缺少 `type` 时继续严格解析，并由严格 `Project` 加载前执行的
  `config migrate` 补充 `type = "http"`

## 文档体验重构

### 事实与治理

- [x] 审计最新 tag、当前 main、待发布 changeset 与未完成能力
- [x] 记录当前 CLI、配置、plugin、MCP、CI/publish 能力与验证状态
- [x] 建立独立文档重构 PRD，并从 Rust PRD 委托文档站范围
- [x] 建立已发布、下一版本与计划中内容的 availability 规则和展示组件

### Fumadocs 纵向切片

- [x] 使用 Next.js、Fumadocs Core/UI/MDX 与 Tailwind 4 替换 Rspress
- [x] 建立英文无前缀、中文 `/zh` 的显式静态路由
- [x] 实现静态 ZBSearch、`llms.txt` 与 `llms-full.txt`
- [x] 完成首页、Introduction、First Release 与 CLI reference 样例
- [x] 验证 GitHub Pages deep link、404、移动端与双语搜索

### 体验纠偏

- [x] 将首页定位改为“跨语言单仓库的版本与发布”，不再以 release plan 或非交互调用作为产品主叙事
- [x] 使用 Tailwind utilities 重构首页，并以官方品牌图形紧凑展示内置 ecosystem
- [x] 在首页和文档页恢复版权、许可、仓库链接完整的全站页脚
- [x] 修复 MDX 容器中的强调标记渲染，并让首发教程默认展示交互式使用路径
- [x] 补齐 plugin、configuration 与中文术语表的首批双语页面
- [x] 建立中文术语规则：中文解释优先，首次出现时保留英文检索词，不使用英文 fallback 代替解释
- [x] 将已发布基线更新为 `0.3.0-rc.6`，并把 plugin runtime/SDK 从下一版本改为已发布
- [x] 为 first release 明确 GitHub Actions 默认路径，修正文档中的 changeset 文件名表达
- [x] 修复小尺寸导航 logo、首页生命周期图、窄屏溢出、语言切换与 smooth-scroll 告警
- [x] 新增逐命令行为文档，并把 CLI 参数参考迁移到命令行模块
- [x] 将命令行侧栏改为双语任务名称，并移除中文术语页对当前语言的多余强调
- [x] 公开文档默认推荐 latest，仅在兼容性边界标记功能起始版本

### 内容重写

- [ ] 重写 getting started 与完整 release workflow（包括既有单仓库接入）
- [x] 补齐 workspace、四个内置 ecosystem 与跨生态依赖；首页只保留紧凑概览
- [ ] 补齐 configuration、release channel、changelog template 与 publish pre-check
- [ ] 重写 GitHub Actions、workflow outputs 与当前 MCP changeset CRUD
- [x] 完成命令行模块的 `init`、`commit`、`config`、`status`、`version`、`publish`、`ci`、`mcp` 与参数参考
- [x] 为已发布 plugin runtime/SDK 编写文档，并明确 Vite plugin 尚未完成
- [ ] 同步完成英文与中文内容，不依赖 fallback

### 质量与迁移

- [ ] 增加 `docs:check`、链接、locale parity、CLI drift 与示例 fixture 检查
- [x] 建立旧 URL inventory 与静态 redirect pages
- [x] 将部署产物切换为 `docs/out`，部署前运行当前 docs checks
- [ ] 删除 Rspress 专用依赖、配置、组件与旧内容
- [x] 创建并验证 `@semifold/docs` changeset
- [x] 运行 production build、静态 direct-route smoke test 与 `smif status`

## GitHub 诊断覆盖

统一诊断已接入 CI PR、status 查询/评论、publish Release/附件和 changelog PR 元数据路径；
已同步双语文档并新增回归测试代码，格式、locale key 一致性及 changeset 静态校验通过。

- [x] GitHub Actions [Unit Test #34055233369](https://github.com/noctisynth/semifold/actions/runs/34055233369) 通过 workspace 编译、N-API 冒烟测试、格式、Clippy 与 Rust 测试，本地未编译。
- [x] 为遵守本地禁止编译约束，使用已安装的 `smif status` 替代 Cargo 启动：解析 1 个 changeset，规划 semifold、semifold-engine、semifold-changelog、@semifold/docs 及依赖传播的 @semifold/cli 共 5 个 patch 升级。

## Agent 文档入口

- [x] 实现同源分语言索引、逐页 Markdown 路由与 HTML alternate，投影逻辑测试通过；静态导出另行验收。
- [x] 完成双语 Agent 指南、Semifold Skill 和仓库贡献入口。
- [x] Bun 测试验证格式化逻辑、Skill frontmatter、引用和 locale parity；使用已安装的 `smif status` 验证独立 changeset，未编译 Rust。
- [x] GitHub Actions [文档 CI #34284510232](https://github.com/noctisynth/semifold/actions/runs/34284510232) 通过类型检查、测试、Next 静态构建与生成产物校验；Bun 专用测试使用 `.mjs`，本地未编译。
