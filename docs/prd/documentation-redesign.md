# Semifold 文档体验重构方案

## 1. 文档状态

- 状态：Accepted，阶段 1 纵向切片已实现
- 更新时间：2026-08-10
- 范围：文档站技术架构、信息架构、内容规范、双语策略、迁移与质量保障
- 事实来源：本 PRD 定义文档产品；Semifold 的领域与行为仍以
  [Rust 架构重设计方案](rust-architecture-redesign.md) 和已验证实现为准
- 目标发布：文档站跟随当前已发布版本；尚未发布的 `main` 能力必须明确标注，不能伪装成当前稳定用法

### 1.1 文档与变更治理

任何影响文档信息架构、公开路由、双语策略、搜索、部署或内容事实来源的变更，必须先更新本 PRD。
根目录 `TODO.md` 只记录本 PRD 与当前文档站之间尚未完成的差异。

纯内容修改不需要 changeset。迁移框架、依赖、构建、部署或测试能力会影响 `@semifold/docs` package，
必须为该独立任务创建新的 changeset，并使用 `.changes/config.toml` 中的 package 名称。

## 2. 2026-08-10 开发事实基线

### 2.1 发布状态

- 最新 Semifold tag 为 `semifold-v0.3.0-rc.6`；`@semifold/plugin-sdk-v0.1.0-rc.0` 与该版本同时发布。
- 当前领域实现复核到远端基线 commit `50af846`；其后的本地提交只处理文档设计，不改变 Semifold 的公开命令面。
- `50af846` 同时具有 `semifold-v0.3.0-rc.6`、`semifold-resolver-v0.4.0-rc.3`、
  `semifold-engine-v0.2.0-rc.4` 与 `@semifold/plugin-sdk-v0.1.0-rc.0` 等发布标签；仓库内插件 runtime、SDK
  与当前 MCP changeset CRUD 已进入已发布基线，不得继续标记为“下一版本”。
- 本轮体验复核前只剩文档迁移 changeset；`smif status` 验证该 changeset 只提升 `@semifold/docs`。

文档重构期间，安装与默认工作流必须描述最新已发布版本。面向下一 RC 的页面允许提前编写，但必须满足
下列条件之一：

1. 页面显式标注“下一版本”；
2. 页面在对应 package 发布后才进入生产导航；
3. 示例同时说明已发布版本与 `main` 的差异。

面向用户的生产文档默认推荐并描述 `latest`，不得在首页、教程、命令参考和同一功能的多个子页面反复声明
当前精确 tag。只有兼容性边界确实影响用户选择时，才标记“从 v0.x.x 开始”或 “Since v0.x.x”；
当前 0.3.0 预发布周期内已经发布的能力统一写作 `v0.3.0`，不暴露 `rc` 序号。安装示例默认获取最新版本，
精确预发布 tag 只保留在本 PRD 的事实审计与实施历史中。

### 2.2 已发布能力

`0.3.0-rc.6` 已包含并可作为默认文档事实：

- Rust、Node.js、Python 与 C++ 内置 ecosystem；
- changeset 创建、合并 bump 与 changelog 分类；
- `status`、`version`、`publish` 和 `ci` 的共享 release/publish plan；
- `config sync`、`config migrate` 和 release channel 管理；
- 跨生态显式 `depends-on`、确定性拓扑排序与依赖环诊断；
- 自定义 changelog release/changeset 模板；
- typed HTTP/command publish pre-check 与 retry；
- GitHub Release、asset 上传与 per-package `github-release` 策略；
- GitHub Actions version/publish 结构化 outputs；
- 可完整参数化的 `init` 与 `commit` 非交互路径；
- TTY/CI、自适应 Unicode 表格、dry-run 与部分失败恢复反馈。
- 开放 `EcosystemId` 与 repository-local JavaScript ecosystem plugin；
- Boa 单文件 ESM runtime；
- 按 glob 授权的只读文件 capability；
- default-deny、按 exact HTTPS origin 授权的 fetch capability；
- plugin discovery、inspection、plan-edits 与 config sync/version 集成；
- `@semifold/plugin-sdk`、Rust 协议生成的 TypeScript wire types 与 drift 检查；
- MCP `get_changeset`、`create_changeset`、`update_changeset`、`delete_changeset`；
- MCP optimistic SHA-256 revision、dry-run、panic isolation 与 lazy project load；
- strict project load 前执行 `config migrate`，以及 Node.js channel/dist-tag warning；
- registry 版本已存在时继续补建缺失 GitHub Release；
- npm trusted publishing/OIDC 与 init workflow 模板保留修复。

### 2.3 已合入 `main`、等待下一次发布的能力

当前没有需要进入本轮生产导航、但只存在于 `main` 且尚未发布的公开能力。后续若出现这类差异，仍按 2.1
的 availability 规则处理。

### 2.4 尚未完成、不得写成可用能力

- 配套 Vite plugin 的单文件 ESM bundle；
- 对残留 import、Node.js builtin、动态 module load 和不支持 Web API 的构建期检查。

在该能力完成前，plugin 文档只能要求用户自行提供无 runtime import 的单文件 ESM；不得展示尚不存在的
`vite` 命令或 package。

### 2.5 验证状态

基线审计时：

- `cargo test --workspace --all-features`：346 项通过；
- `@semifold/plugin-sdk` protocol/Biome 检查通过；
- SDK 类型测试与 3 项 runtime 测试通过；
- 现有 Rspress production build 通过；
- `smif config sync --check` 与 `smif config migrate --check` 通过；
- 工作树干净。

## 3. 现状诊断

当前 Rspress 文档按 CLI 表面结构组织，只有 Start、Commands、Configuration 与 Advanced 四类内容。
它没有先建立 release workflow 心智模型，也没有区分 workspace owner、contributor、release engineer 与
plugin author 的任务。

已确认的问题包括：

- Quick Start 在同一页混合安装、初始化、配置、changeset、version、publish 与 CI；
- 用户在理解 Semifold 之前先面对多个安装渠道和完整配置；
- `config`、release channel、workflow outputs、模板与失败恢复等已实现能力缺失；
- 旧配置页仍描述 legacy `version-mode`、snake_case 字段与过期 pre-check 形状；
- MCP 页面仍描述已经删除的 `get_tags`、`get_packages` 和旧 `create_changeset` surface；
- 桌面导航重叠、首页 emoji 缺字、移动端代码被截断；
- 公开页面暴露 `/index` 链接，部分直接访问返回 404；
- 部署只验证 build，不验证链接、双语完整性、静态深链、移动端或内容示例。

迁移不得复制这些页面结构或逐页转换旧 MDX。

## 4. 目标与非目标

### 4.1 主要目标

1. 让首次用户先理解 Semifold 如何统一跨语言单仓库的版本与发布，再接触配置细节。
2. 为 workspace owner、contributor、release engineer 和 plugin author 提供独立成功路径。
3. 以真实 CLI、配置类型、fixture 和 release plan 约束文档事实。
4. 保留英文无前缀与中文 `/zh` 的公共 URL 语义。
5. 在 GitHub Pages 上提供完全静态的双语站点、搜索、SEO 与 LLM 文本入口。
6. 让深层路由、链接、双语覆盖与关键示例进入 CI。
7. 限制定制边界，避免 fork Fumadocs 核心布局造成长期维护负担。

### 4.2 非目标

- 第一阶段不实现需要服务端 runtime 的 Ask AI 或内建 feedback backend；
- 第一阶段不提供历史版本切换器；
- 不从 Rust 内部类型直接生成全部解释性内容；
- 不承诺未发布、未完成或只存在于 PRD 的能力；
- 不把首页改造成与文档无关的独立营销网站。

## 5. 用户与核心任务

### 5.1 Workspace owner

- 判断 Semifold 是否适合现有 monorepo；
- 初始化、检查 package discovery，并维护配置漂移；
- 建立跨生态依赖、发布通道和 changelog 策略；
- 配置 GitHub Actions 与 registry 权限。

### 5.2 Contributor

- 为一次变更创建正确的 changeset；
- 选择 package、bump 与 changelog tag；
- 在提交前读懂 `smif status`。

### 5.3 Release engineer

- 理解 release PR、version、publish 和 registry preflight；
- 消费 GitHub Actions outputs；
- 诊断 partial failure、already-published package 与 Forge/asset 状态。

### 5.4 Plugin author

- 判断内置 ecosystem 是否已经满足需求；
- 编写 schema v1 JavaScript plugin；
- 配置 read patterns、SHA-256 pin 与 HTTPS origins；
- 使用 TypeScript SDK，并自行生成首版要求的单文件 ESM；
- 理解 plugin 不能执行的 privileged operations。

## 6. 核心心智模型

Semifold 的产品心智模型是“一座仓库、一张软件包关系图、一套版本与发布流程”。内置生态适配器与插件先把
不同清单格式归一为同一张工作区图；变更集、版本联动、变更日志和发布再围绕这张图工作。

所有入门和工作流内容围绕下列用户路径：

```text
code change
  -> changeset
  -> repository-wide version decision (`smif status`)
  -> version edits and release pull request
  -> dependency-ordered publish plan
  -> registry / Forge report
```

`release plan` 是实现可审查性的公开结果，不是 Semifold 的产品定位，也不能在首页替代跨生态版本管理本身。

必须在进入完整配置参考前解释：

- PackageId 与 manifest name 的区别；
- ecosystem adapter/plugin 负责 discovery、inspection 与 edit planning；
- changeset 表达发布意图，tag 只分类 changelog，不决定 release channel；
- `status` 与 `version` 使用同一个 plan；
- `publish` 使用当前 manifests/changelogs 构造独立 publish plan；
- manifest dependency 与显式 `depends-on` 的传播语义不同；
- stable/named channel 是 package 发布状态，不是 changelog tag。

## 7. 信息架构

英文为无 locale 前缀的 canonical 内容，中文使用相同 slug 并加 `/zh`。

```text
/
/docs/
  introduction
  concepts/
    glossary
  getting-started/
    installation
    first-release
    adopt-existing-monorepo
    github-actions
  workflow/
    changesets
    preview-release
    version
    release-pull-request
    publish
    recover-failures
  workspace/
    package-discovery
    config-sync
    dependencies
    rust
    nodejs
    python
    cpp
  versioning/
    bump-rules
    release-channels
    changelogs
  automation/
    github-actions
    workflow-outputs
    mcp
  commands/
    index
    init
    commit
    config
    status
    version
    publish
    ci
    mcp
    reference
  plugins/
    overview
    quick-start
    protocol
    capabilities
    configuration
  configuration/
    overview
    packages
    resolvers
    publish-hooks
    pre-checks
    templates
  reference/
    configuration
    changeset-format
    template-contexts
    environment
  troubleshooting/
```

首页不进入 docs sidebar。它只负责：

- 用一句具体价值表达说明 Semifold 是跨语言单仓库的版本与发布工具；
- 用仓库中的多种清单文件展示统一的版本、依赖与发布生命周期；
- 给出“第一次发布”和“理解工作流”两个主要入口；
- 紧凑展示内置生态，并把插件扩展作为与内置能力同级的产品能力；
- 链接当前版本、GitHub 与双语入口。

首页只说明 Semifold 内置支持 Rust、Node.js、Python 与 C++，并可通过插件扩展其他生态；不得在首页展开各生态的
清单格式、发现范围、版本写入方式、依赖规则或能力限制。这些技术事实统一由 `workspace` 模块承载，并在对应生态
页面中分别说明。

首页不得把某一次架构重构（例如不可变计划）或受限环境兼容能力（例如完整参数化的非交互调用）写成产品主卖点。

命令行模块用于查询单条命令的职责、输入、默认交互路径、副作用、失败语义与完整参数，不取代围绕用户任务组织的
getting started、workflow、workspace 和 automation。原 `/docs/reference/cli` 保留静态迁移入口，规范位置改为
`/docs/commands/reference`。

命令行模块的侧栏和页面标题使用本地化的任务名称，例如“初始化”“配置文件”“持续集成”和“MCP 工具”；
`init`、`config`、`ci` 等稳定 slug 与正文中的 `smif ...` 命令保留原文。导航负责帮助用户按意图选择页面，
命令拼写和完整参数由页面正文与参数参考承载。

## 8. 页面与写作规范

### 8.1 页面类型

- Tutorial：从可复现起点获得完整结果；
- How-to：解决一个明确任务；
- Explanation：解释领域规则和权衡；
- Reference：完整、精确、可扫描的公开契约。

一个页面只能有一个主要意图。不得把 tutorial 与完整 reference 混在同一页面。

### 8.2 页面结构

面向任务的页面依次包含：

1. 读完能得到什么；
2. 前置条件；
3. 最短默认路径；
4. 命令或配置；
5. 预期输出和产生的文件；
6. 常见失败与恢复；
7. 下一步。

默认路径先出现；操作系统、package manager 与 ecosystem 变体使用共享且持久化的 Tabs。代码块必须在
窄屏内部滚动，不得让页面产生水平滚动。

### 8.3 术语与 CLI

- 面向用户统一使用 `smif`；第一次出现时说明 `semifold` 是等价长名称；
- command、flag、配置 key、package ID 与路径保留原文；
- 中文正文优先使用清楚的中文概念，并在首次出现时用括号保留英文检索词，例如“变更集（changeset）”、
  “发布计划（release plan）”、“软件生态（ecosystem）”、“清单文件（manifest）”和“软件包仓库（registry）”；
- `PackageId`、配置字段和协议 operation 等精确标识保留原文，但必须立即解释它在用户任务中的含义；
- 不允许连续堆叠未解释的英文领域词，也不允许把英文原词当作中文解释的 fallback；
- 页面标题、描述和导语直接说明内容，不用“用中文解释”“中文文档会”等自我指涉来强调当前页面语言；
- 不使用“zero pain”“stable”等无法验证的绝对营销表述；
- 每项 ecosystem 能力分别说明 discovery、version edit、dependency propagation 与 publish，而不是一个总状态。
- 英文首页避免把 `polyglot` 当作无需解释的主标题词；优先使用 “multiple languages and package ecosystems”，
  在解释性正文首次出现时才说明 `polyglot` 是常见同义词。
- 启用 GitHub Actions 的仓库在 first-release 默认路径中只需要提交 changeset，并合入自动维护的 release PR；
  手工 `version` / `publish` 作为理解机制或没有工作流时的替代路径，不能让用户误以为两套流程都必须执行。

## 9. 内容事实来源

事实优先级从高到低为：

1. 已发布 tag 对应的公开 CLI 和配置行为；
2. 当前 PRD；
3. 已通过的领域/fixture/CLI 测试；
4. Rust 类型与公开 TypeScript SDK；
5. changeset 与 changelog；
6. 手写解释。

CLI reference 必须由 `clap` command surface 生成或由快照检查约束。配置示例必须能被当前 loader 解析；
关键工作流示例必须在 fixture 中运行。文档不得以 README、旧文档或 TODO 作为独立事实来源。

## 10. 技术架构

### 10.1 框架

`docs` package 使用：

- Next.js 16；
- Fumadocs Core、UI 与 MDX；
- React 19；
- Tailwind CSS 4；
- Bun canary workspace；
- TypeScript strict mode。

首页、页脚和简单布局优先使用 Tailwind utilities。全局 CSS 只保留主题 token、Fumadocs/MDX 需要的跨组件规则、
无法由局部 utility 清楚表达的主题切换，以及确有复用价值的领域组件样式；不得为单个首页复制一套独立 CSS 设计系统。

不采用 Fumapress/Waku；当前需求需要对路由、静态导出、双语和首页进行明确控制。也不采用 Astro + React
islands，避免再引入一层 UI runtime 边界。

### 10.2 静态部署

Next.js 使用 `output: 'export'`，部署产物为 `docs/out`。GitHub Pages workflow 通过显式配置
`bun-version: canary` 的 Bun 安装锁定依赖并执行检查，不运行 Node server。
静态导出必须验证：

- 根首页；
- 英文与中文 docs 首页；
- 至少一个三层深度页面；
- `/api/search` 静态 JSON；
- `llms.txt` 与 `llms-full.txt`；
- 404 页面。

是否启用 `trailingSlash` 由纵向切片通过 GitHub Pages 目录行为决定；验收要求旧 clean URL 和新 canonical
URL 都能直达，不以本地 Next dev server 行为代替验证。

### 10.3 国际化路由

- 英文保持 `/docs/...`；
- 中文保持 `/zh/docs/...`；
- 首页为 `/` 与 `/zh/`；
- 静态站不使用 cookie locale 或 middleware locale negotiation；
- 两个显式 route tree 共享同一 page renderer；
- Fumadocs i18n fallback 设为 `null`；
- 每个页面输出 canonical、`hreflang="en"`、`hreflang="zh-CN"` 与 `x-default`。
- 语言切换必须按同一 slug 显式映射：英文不加 locale 前缀，中文只加 `/zh`；不得依赖 Fumadocs 默认的
  `/{locale}` 前缀拼接，也不得产生 `/en/docs/...`。

### 10.4 搜索

使用 Fumadocs built-in ZBSearch 静态模式：

- server route 使用 `staticGET`；
- UI 使用 `staticClient`；
- 查询按当前 locale 过滤；
- 中英文正文、标题与 heading 均进入索引；
- 搜索 index 大小进入构建报告，超过约定预算后再评估 cloud search。

### 10.5 LLM 文本

Fumadocs MDX 开启 processed Markdown 输出，并静态生成：

- `/llms.txt`：页面索引；
- `/llms-full.txt`：完整公开文档；
- 逐页 Markdown route 按第 18 节从同一 processed MDX 内容静态生成。

下一版本或未发布页面不得混入默认 `llms-full.txt`，除非文本中明确携带 availability。

## 11. 视觉与组件边界

- 使用 Fumadocs DocsLayout、DocsPage、search dialog 和默认可访问性行为；
- 只通过 design tokens、全局样式、首页与少量领域组件定制；
- 不复制或 fork Fumadocs sidebar、search、mobile nav 的内部实现；
- 保留现有 Semifold logo；内置生态展示使用可识别、带无障碍名称的官方品牌图形，不使用字母切片或 emoji 充当 logo；
- 导航中的小尺寸品牌图形使用为实际显示尺寸设计的无阴影 SVG，不得缩放 README 使用的细线大图造成模糊边缘；
- 首页功能区在常见桌面首屏内保持紧凑，内容量少时不得用固定最小高度制造大块空白；
- 生命周期等具有阶段关系的内容渲染为可访问、可换行的真实图示，不使用带复制按钮的 `text` 代码块模拟图；
- 320px、360px 与 390px 宽度下，中文长标题必须允许自然断行，所有 grid/flex 子项必须可收缩，页面不得整体横向溢出；
- 正文目标宽度约 720–800px，reference table 可在容器内滚动；
- 提供浅色/深色主题，并尊重系统偏好；
- 交互目标、焦点可见性和颜色对比满足 WCAG 2.2 AA。

首页与所有文档页底部都必须显示版权与 AGPL-3.0-only 许可声明，并提供仓库和许可链接。

领域组件限制为具有明确复用价值的内容，例如 ReleaseFlow、EcosystemMatrix、ExpectedOutput 与
Availability。纯装饰效果不新增自定义 runtime 组件。

## 12. 迁移与 URL 策略

先建立旧 URL inventory。每个旧 URL 必须归入：

- 保留原内容语义；
- 合并到新页面；
- 拆分后跳转到主要 successor；
- 明确废弃。

GitHub Pages 不提供通用 HTTP 301。优先保留仍合理的 slug；无法保留时生成静态 redirect page，包含
canonical、可见链接和无脚本可用说明。不得只使用客户端 router redirect。

旧 Rspress 内容只作为事实线索，不逐文件搬迁。新页面完成并通过双语与事实检查后再删除对应旧页面。

## 13. 质量保障

新增 `docs:check`，至少执行：

- TypeScript 与 MDX 类型检查；
- Fumadocs type generation；
- Biome check；
- 内部 URL、heading anchor 与 MDX component link 检查；
- 英中页面、meta 与导航 parity；
- CLI reference drift；
- config/changeset 示例 fixture；
- Next static production build；
- 对 `out` 启动静态服务器后的 direct-route smoke test；
- 390px 与 1440px 的关键页面 layout smoke test；
- locale search smoke test。

CI 中 docs job 与 Rust tests 分离，避免文档依赖安装拖慢纯 Rust 反馈。部署 job 必须依赖 docs check 成功，
并上传 `docs/out` 而不是 Rspress `docs/doc_build`。

## 14. 实施阶段

### 阶段 0：事实审计与 PRD

- 确认 release/main/pending 边界；
- 记录公开 CLI、配置、plugin、MCP 和发布能力；
- 更新本 PRD 和 TODO。

### 阶段 1：纵向技术切片

- 替换 docs package 依赖与 scripts；
- 建立 Next/Fumadocs shell；
- 完成首页、Introduction、First Release 与 CLI reference 样例；
- 验证静态双语路由、搜索、LLM、GitHub Pages 与移动端。

### 阶段 2：核心成功路径

- 重写 installation、first release、adoption、changeset/status/version/publish/CI；
- 加入 expected output 与 failure recovery；
- 完成英文后同步中文，不允许长期 fallback。

### 阶段 3：Workspace、配置与自动化

- 补齐四个内置 ecosystem；
- 补齐 config sync、dependencies、channels、templates、pre-check 与 workflow outputs；
- 重写 MCP 为当前 CRUD surface。

### 阶段 4：Plugin 文档

- `0.3.0-rc.6` 与 `@semifold/plugin-sdk 0.1.0-rc.0` 发布后，将 runtime、SDK 和 protocol 页面标记为 released；
- 文档只覆盖 runtime 已实现能力；
- Vite plugin 完成并发布后再补写 bundle 工作流。

### 阶段 5：迁移与切换

- 完成旧 URL map 和 redirect pages；
- 运行所有 docs checks；
- 更新部署 workflow；
- 删除 Rspress 专用配置与组件；
- 生产环境 smoke test。

## 15. 验收标准

- 不熟悉 Semifold 的用户不读源码即可完成初始化、创建 changeset，并解释 `smif status`；
- 所有生产导航内容对应已发布能力，下一版本内容具有明确 availability；
- 当前 CLI command/flag、配置 key、MCP tool 和 plugin protocol 不再引用旧 surface；
- 每个旧公共 URL 能访问等价内容或静态跳转；
- 英中页面与导航集合完全一致；
- 中英文搜索只返回当前 locale 的正确结果；
- 390px 页面无整体水平滚动，代码块和宽表格只在自身容器滚动；
- 中英文任意同 slug 页面之间切换后目标路由存在，英文目标不包含 `/en`；
- 页面切换时不得产生 Next.js `missing-data-scroll-behavior` 告警；启用全局 smooth scrolling 时，根 `<html>`
  必须声明 `data-scroll-behavior="smooth"`；
- 所有深层页面可从静态服务器直接访问；
- `docs:check` 和 production build 在 CI 中通过；
- Lighthouse Accessibility 不低于 95，Performance 不低于 90；
- 生产部署不依赖 Node server、cookie locale 或运行时 rewrite。

## 16. 已确定决策

- 使用 Next.js + Fumadocs，不使用 Fumapress/Waku；
- 第一阶段继续使用 GitHub Pages 静态部署；
- 英文无前缀，中文使用 `/zh`；
- Fumadocs 语言选择器使用本站自定义 locale path 映射，不使用默认 `/en` 前缀行为；
- 使用静态 ZBSearch；
- 保留现有 logo，重做首页结构但不先做品牌重设计；
- 首页与用户文档描述已发布版本，下一版本功能显式标注；
- plugin runtime/SDK 与 Vite bundler 分开描述；
- 命令行独立成查询模块，workflow 继续按用户任务组织；
- 第一阶段不加入需要服务端的 AI chat 或 feedback backend。

## 17. 当前实施状态

截至 2026-08-10，阶段 0 与阶段 1 已完成：

- docs package 已迁移到 Next.js 16、Fumadocs Core/UI/MDX、React 19 与 Tailwind CSS 4；
- 英文 `/docs` 与中文 `/zh/docs` 使用两个显式静态 route tree；
- 首页、Introduction、First Release 与 CLI reference 样例已完成英中双语重写；
- 静态 ZBSearch、`llms.txt`、`llms-full.txt`、canonical 与 hreflang 已生成；
- `docs:check` 已覆盖源码生成、TypeScript、locale 文件 parity、静态构建、必需输出、静态内部链接与 locale search 配置；
- 本地静态服务器的根路由、三层深路由、中英文路由、search/LLM 输出与 404 smoke test 通过；
- 1440px 首页与 390px 深层教程完成真实 Chromium 截图检查。
- `21e08e6` 消费功能 changeset 后，本地两项独立文档任务各自保留 changeset；当前发布计划为 2 个 changeset、
  1 个 package，`@semifold/docs` 计划从 `1.1.0-beta.0` 提升到 `1.1.0-rc.0`。
- 旧 Rspress URL 已建立 inventory，并生成 canonical、可见链接和 HTML refresh 完整的静态 redirect page；Pages workflow 已切换为先运行 `docs:check`，再上传 `docs/out`。

首次用户体验复核后，阶段 2 至阶段 4 已完成一组纠偏切片：

- 首页产品定位改为跨语言单仓库的版本、依赖、变更日志与发布管理，不再以 release plan 或非交互调用作为主叙事；
- 首页使用 Tailwind utilities 和 Rust、Node.js、Python、C++ 官方品牌图形，生态区域改为紧凑列表，并把插件提升为同级入口；
- 首页与文档页恢复版权、AGPL-3.0-only、仓库和许可链接完整的全站页脚；
- First Release 恢复交互式默认路径，自动化参数降为后置补充，并修复 MDX 容器中 Markdown 强调符号的字面量渲染；
- 新增 installation、configuration overview/reference、plugin overview/quick-start/capabilities 与 glossary，
  并进一步新增逐命令说明与 CLI 参数参考；当前英中各 28 个内容/meta 文件，静态构建生成 89 个页面；
- 中文写作改为中文解释优先、英文检索词首次出现时括注，术语表覆盖仓库、软件包、生态、版本与发布概念；
- 首页定制 CSS 已迁移到组件内 Tailwind utilities，全局样式从 558 行收敛为 37 行，只保留框架导入、基础规则和跨组件行为。
- 已发布基线更新为 `0.3.0-rc.6` 与 `@semifold/plugin-sdk 0.1.0-rc.0`，plugin runtime/SDK/protocol
  改为 released，只有尚未实现的 Vite bundler 继续标记为 planned；
- 首页小尺寸品牌图改用无阴影 favicon SVG，生命周期改为可访问的响应式图示；320px、360px 与 390px
  Chromium 验证均满足 `scrollWidth === clientWidth`；
- Fumadocs 语言切换使用自定义 canonical path 映射，中文 first-release 切换英文后到达
  `/docs/getting-started/first-release/`，不再生成 `/en/docs/...`；
- 根 `<html>` 声明 smooth-scroll 行为，真实页面切换的控制台检查没有 scroll behavior、404 或运行时告警；
- 命令行成为独立查询模块，包含 `init`、`commit`、`config`、`status`、`version`、`publish`、`ci`、`mcp`
  与参数参考；workflow 内容仍按用户任务组织。

本轮新增第三个独立文档 changeset；`smif status` 验证 3 个 changeset 仍只影响 `@semifold/docs`，计划从
`1.1.0-beta.0` 提升到 `1.1.0-rc.0`，计划指纹为 `8fc3d5a6632c`。

阶段 2、阶段 3 的其余工作流、workspace、自动化内容与阶段 5 的内容清理仍在进行。旧 Rspress 内容和配置暂时保留为事实核对与迁移线索；生产构建已经切换到 Fumadocs，但当前导航仍未覆盖完整产品内容，不能被解释为全部重写已经结束。

## 18. Agent 文档与 Skill

- 公开页面直接面向开发者，说明命令用途、操作步骤、返回结果和必要的失败恢复。任务指南同样使用
  简洁的操作说明；不把内部实现取舍、审查推断、假设性风险或泛化的防御性告诫写入用户正文。
  保留影响使用结果的事实，例如 dry-run 执行范围、已有输出目录处理和发布失败后的部分状态。

- `/llms.txt` 保持一个 H1，以英文文档分组索引为主体，并链接 `/zh/llms.txt`、全文和 Skill。
- `/zh/llms.txt` 单独索引中文内容，禁止 fallback；原 `/llms-full.txt` 保留双语全文兼容行为。
- `/markdown/{en,zh}/{slug}.md` 从同一 Fumadocs processed Markdown 生成，根文档为 `index.md`；
  保留标题、描述、原始网页 URL 和语言，正文内部相对链接解析为原网页对应的绝对 URL。
- 网页 metadata 提供 `alternate` / `text/markdown` 链接；索引直接链接 Markdown，按现有任务模块分组。
- 增加双语 Agent 使用说明，说明入口、版本适配、Skill 获取和操作的前置条件、输出、副作用及恢复路径。
- 仓库分发 `skills/semifold/SKILL.md`，元数据满足 Agent Skills 格式；正文仅保留适用范围、
  本地 CLI 前置条件、`skill list` → `skill get <task>` 发现协议、同一可执行文件执行和授权边界。
  不固定任务 ID、业务 flags 或版本相关恢复规则；版本/可执行文件变化时刷新指引，发现协议不支持时
  报告兼容性问题，不猜测任务或使用其他版本指引。安装包只有该文件，不再依赖旁边的 references。
- CLI 提供 `smif skill list`、`smif skill get <task>` 和 `smif skill export --output <dir>`。
  `skill --language en|zh` 可显式选择内容语言，省略时将系统中文 locale 归一为 zh，其余为 en。
  成功 stdout 为单个 JSON 对象：`protocolVersion: 1`、`cliVersion`、`language`、`data`。
  list 的 data 是 `{task, summary}` 数组；get 是 `{task, commands, instructions}`，其中 commands
  是可用于 `<command> --help` 的命令路径字符串，instructions 合并共享规则与所选任务指南。
  export 的 data 是 `{output, files: ["SKILL.md"], written}`；dry-run 时 written 为 false。
  错误沿用 CLI stderr 和非零退出码，stdout 不混入 banner、终端样式或状态提示。
  发现协议的基础字段和调用形态保持兼容，业务任务列表通过 list 动态发现，不承诺永久固定。
- list/get 在项目发现之前执行，离线、无配置 I/O、无 hook、无需 Git 或初始化；export 仅写显式输出
  目录，不自动安装客户端技能、不覆盖已有路径（包括悬空符号链接）。父目录必须存在，可通过链接
  解析为规范绝对路径，输入及解析结果必须是有效 Unicode；output 返回解析后的绝对路径。
  写入失败保留已创建目录和可能部分写入的文件，供检查后选择新目录重试，不递归清理或删除可能被
  并发替换的内容。全局 dry-run 不写文件。命令参数细节继续由已安装版本的 --help 提供，不新增独立 schema 子系统。
- 双语 `docs/content/docs/{en,zh}/agent-tasks/*.mdx` 是任务指南唯一手写来源；初始覆盖 init、config、
  changeset、status、release 与共享规则，元数据声明任务 ID、相关命令，description 提供任务摘要。
  维护脚本生成 crate 内的任务索引、最小 Skill 副本，以及 en/zh locale 文件的标记生成区块；
  指南正文和摘要同样通过 `t!` 读取，不在 Rust 中硬编码用户文案。生成产物提交并随 crate 发布，
  Cargo 构建直接读取产物，locale 文件显式纳入生产库编译依赖，正文修改后增量构建必须更新内嵌翻译。
  构建不启动文档站或依赖 Bun；运行时不读取网站或新版本源码。
  生成器拒绝双语任务/命令不一致和不支持的 MDX 内容，检查模式只报漂移、不修改产物。
- `AGENTS.md` 增加文档源与 Skill 的职责、开发验证入口，不把使用者的 Skill 作为仓库贡献规则。
- 使用纯格式化函数隔离路由依赖；测试覆盖单 H1、语言隔离、Markdown URL 映射、正文/链接保留；
  静态产物检查覆盖索引中的全部 Markdown 链接、双语路由与 HTML alternate。Skill 校验检查
  frontmatter、单文件分发、任务生成漂移和未完成占位符。CLI 测试覆盖无仓库/坏配置读取、两种语言、
  版本元数据、实际响应与 locale 源正文一致性、未知任务错误、相关命令解析、导出/已有路径保护及
  dry-run 无写入；导出内容保留内嵌文件原字节，测试兼容 LF 与 CRLF 检出。
  用户禁止本地编译时仅执行脚本测试和静态校验，构建验证待 CI。

- Bun 专用测试采用 `scripts/*.test.mjs`，通过 `bun test` 执行；不混入 Next 应用的 TypeScript 类型环境，不为测试全局声明扩大生产类型范围。
