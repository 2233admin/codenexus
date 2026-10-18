# CodeNexus 外部依赖分层 (criticality, 不是 alphabetical)

更新于 2026-10-18（#23，主人确认）：CodeNexus 改用 CodeGraph 作为解析和符号图引擎，
不再自写 tree-sitter 管线，也不再从 sentrux lift 语言抽取或结构治理代码。

## ENGINE UPSTREAM

**CodeGraph**（`github.com/colbymchenry/codegraph`，MIT）：负责 tree-sitter 符号/调用图、
索引、callers/callees/impact/affected。CodeNexus 只消费它的结果，不 fork 解析器，
也不共享它的 SQLite。

## CodeNexus 自己负责的部分

- **Git 叠加层**：blame、提交历史、churn/热点、co-change（gix）。
- **语义检索**：embeddings，是否保留和时机见 #23。
- 对外仍是 code-intel-pipeline 的 `code-intel-codenexus-port`。

## 不再承担的（已转给别处）

**sentrux**（`github.com/2233admin/sentrux`，MIT）：原 Phase 04.5 的 lift 计划停止。
- LanguageSemantics / lang_extractor 与 CodeGraph 重叠，停止。
- metrics / arch / evo / dsm / rules 属于结构治理，由 code-intel-pipeline 的
  `structure.sentrux` 负责。CodeNexus 不再复制一份。
- 已经 lift 进来的 `codenexus-metrics` 等子 crate 保留 NOTICE 归属；删除与否在实现切片时
  按实际调用决定。

## OPTIONAL PLUGINS / INTEGRATION TARGETS

**obsidian-llm-wiki**、**memU**：Phase 5 Bridge 对端，可选。不安装时核心检索与
Git 叠加层照常可用。

## 含义 (How to apply)

- 谈“重要”时：CodeGraph（引擎）>> Git 叠加层（自有价值）>> obsidian/memU（可选）。
- 不再为解析、符号图或结构度量写新代码。需要这些能力时，向 CodeGraph 或
  code-intel-pipeline 的 sentrux 要。
- 给外人介绍：“基于 CodeGraph 的代码 + Git 智能 provider，补上 blame/热点/语义检索”。
