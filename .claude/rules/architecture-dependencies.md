# CodeNexus 外部依赖分层 (criticality, 不是 alphabetical)

记于 2026-05-03 (Curry zoom-out 更正): 我把 sentrux / memU / obsidian-llm-wiki
都放在 "associated components" 同层 -- **错**。三者 criticality 完全不同。

## CRITICAL UPSTREAM

**sentrux** (`github.com/2233admin/sentrux`, MIT): 整个 Phase 04.5 的核心 lift
源。~8000 LoC 待 port (排除 egui + pro_registry)。CodeNexus 抄它的设计 + 大段
代码 verbatim:
- LanguageSemantics + RepoCtx + lang_extractor 是 multi-lang 路线整个架构骨架
- 04.5-02a 已 lift metrics/arch 进 codenexus-metrics 子 crate
- 04.5-02b/03/05/07 持续 lift evo / dsm / rules / lang_extractors framework
- NOTICE 已挂 sentrux MIT attribution

跟 CodeNexus 关系不是 "调用", 是 "verbatim 抄进 sub-crate"。

## OPTIONAL PLUGINS / INTEGRATION TARGETS

**obsidian-llm-wiki** (`D:/projects/obsidian-llm-wiki/`): Phase 5 Bridge 对端,
markdown wiki-link graph 提取目标 + spike-001 的 7 NL queries 源。**用户不安装
obsidian-llm-wiki 仍能跑 CodeNexus**。

**memU** (`D:/projects/memU`): Phase 5 Bridge 第二对端, `remember_symbol_note`
接口给 (path, name, kind) keyed memory attachment。**memU 不在线 CodeNexus 仍跑**。

## 含义 (How to apply)

- 谈 "重要" 时, sentrux >> memU/obsidian-llm-wiki
- 谈 "功能损失" 时, 卸 sentrux = CodeNexus 半身不遂; 卸 obsidian/memU = 少几个
  特性, 核心搜索 + A2A endpoint 全保
- 写 zoom-out / 架构 map 时, 别把这三个并列 "external deps"; sentrux 单独一层
- Phase 顺序决策: sentrux lift (04.5-02b/03/05/07) 是项目 backbone work; obsidian/
  memU 集成 (Phase 5 Bridge) 是 add-on
- 给外人介绍 CodeNexus 时, 说 "sentrux 的下游 fork + 加了 embedder 和 A2A 服务"
  比 "用 sentrux + obsidian + memU" 准确
