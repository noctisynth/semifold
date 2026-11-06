# Semifold 项目上下文

## 权威文档

- 技术 PRD：[docs/prd/rust-architecture-redesign.md](docs/prd/rust-architecture-redesign.md)
- 实施差异清单：[TODO.md](TODO.md)

PRD 是需求、架构和设计的技术事实来源。开始任何实现前，先检查 PRD 是否已表达该需求；如果需求或设计发生变化，先更新 PRD，再修改代码。

`TODO.md` 只记录 PRD 与当前代码之间的未完成差异：

- PRD 领先于代码时，新增或更新对应 TODO；
- 完成并验证 PRD 中的一项能力后，更新或勾选对应 TODO；
- 不要将 TODO 作为独立需求来源，也不要让 TODO 与 PRD 冲突。

## Rust 与 Cargo 规则

- 禁止手动编辑任何 `Cargo.toml`。
- 新增或移除依赖、crate、workspace member 或 crate 元数据时，必须使用 Cargo CLI，例如 `cargo add`、`cargo remove`、`cargo new` 或 `cargo init`。
- 如果 Cargo CLI 无法表达变更、命令失败，或预期结果不明确，立即停止并向用户确认；不得通过手动编辑 `Cargo.toml` 绕过。
- 当前受限环境中的 `cargo add` 无法访问 crates.io；新增依赖时应直接使用受审批的网络执行 `cargo add`，并将 `prefix_rule` 限定为对应 package 的 `cargo add -p <package>`，不得以手动编辑 `Cargo.toml` 替代。

## 实施约束

- 未经用户明确要求，不直接修改代码或配置文件。
- 领域设计优先：先生成或更新计划、接口和测试策略，再引入副作用。
- 新设计中不恢复全局可变万能 `Context`；渲染使用不可变、按作用域构造的 `ReleaseContext`、`PackageContext`、`ChangelogContext` 与 `TemplateContext`。
- 生产 Rust 代码不得使用会 panic 的 `unwrap()`；外部输入、文件系统、配置和领域查询的失败必须返回可处理的错误。只有类型、构造器或同一函数内穷尽分支已证明的内部不变量可使用 `expect()`，且消息必须说明该不变量；测试中的断言性使用可保留。

- 每次完成任务时，除总结业务和技术产出外，必须明确说明本次采用的技术方案相对既有设计的变动；不得将方案变动隐式包含在实现中而不告知用户。

## 文档同步

- 每次完成代码开发任务后、交付或提交前，必须判断变更是否影响用户可见行为、配置契约、CLI/API、工作流、示例或架构说明。
- 如果存在影响，必须在同一任务中更新对应现有文档；公开文档同时维护英文与中文，不得只更新单一语言或依赖 fallback。
- 如果判断不需要更新文档，必须在最终总结中明确说明原因，不能默认省略文档检查。
- 文档描述必须以当前已验证实现为准；发现技术方案、PRD、代码与文档不一致时，先按权威文档规则处理设计差异，再完成代码和文档同步。

## Changeset

- 完成会影响任一已发布 package 的功能、修复、重构、依赖或测试能力变更后，必须在 `.changes/` 生成对应 changeset；纯文档或不影响 package 的维护工作除外。
- changeset 必须使用 `.changes/config.toml` 中已配置的 package 名称，并选择与变更性质一致的 bump level 和 tag。
- 默认一个独立任务对应一个独立 changeset；同一分支上的后续独立任务必须新建 changeset，不得修改、合并或复用已有任务的 changeset。只有用户明确要求将多个改动视为同一任务时，才可共享 changeset。
- 生成 changeset 后，运行 `cargo run -p semifold --bin smif -- status`，确认其能被解析且发布计划符合预期。

## 国际化（i18n）

- 所有面向用户的 CLI 文案（命令描述、flag help、成功/错误/状态输出和交互提示）必须通过 `rust-i18n` 的 `t!` 宏提供；不得在 Rust 源码中新增硬编码的自然语言文本。
- 实现或修改用户可见文案时，必须同步更新 `crates/semifold/locales/en.toml` 和 `crates/semifold/locales/zh.toml` 中对应的 locale key，并遵循现有 `[cli.*]` 的分组结构；不得依赖英文 fallback 掩盖中文缺失。
- 在提交实现前，检查新增或修改的用户可见路径是否具有 locale key，并确认 `en.toml` 与 `zh.toml` 的 key 集合完全一致；测试断言可使用必要的固定字符串，但不应将其作为生产文案来源。

## 文档与 Skill 入口

- 文档站设计：[docs/prd/documentation-redesign.md](docs/prd/documentation-redesign.md)。
- 双语正文只维护 `docs/content/docs/en` 与 `docs/content/docs/zh`；网页、Agent 索引和逐页 Markdown 从同一源生成。不要编辑旧 `docs/docs` 作为生产内容。
- `skills/semifold/SKILL.md` 面向使用 Semifold 的 Agent，只保留本地 `skill list/get` 发现入口；此文件面向修改 Semifold 仓库的 Agent。
- 版本相关任务指南只编辑 `docs/content/docs/{en,zh}/agent-tasks/*.mdx`；在 `docs/` 运行 `bun run generate:skill`，提交同步生成的 crate assets 和 en/zh locale 标记区块，不手改生成内容。
- 无构建验证：在 `docs/` 运行 `bun run test:agents` 与 `bun scripts/check-locales.mjs`，覆盖文档投影、Skill 入口和生成漂移。
- 完整站点验证：`bun run docs:check` 包含静态构建，仅在用户允许本地构建时运行，否则交由 CI。
