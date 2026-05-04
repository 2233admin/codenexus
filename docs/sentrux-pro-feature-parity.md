# sentrux Pro 解构 -- CodeNexus feature parity 研究

**Author**: Claude Opus 4.7 session 2026-04-29
**Status**: Research / design input for CodeNexus Phase 4+
**Verdict**: sentrux Pro 护城河 = packaging + UI, **不是算法 IP**. CodeNexus 直接吸收 OSS 算法 + OSS 化 Pro 层即可超车.

## 1. 一句话结论

sentrux 卖的 Pro 功能, **5/6 是 free 数据 + 标配工具能在 200 行 Python 内复刻的**. 真正闭源 IP 只有 (a) 32-bit treemap renderer, (b) per-language tree-sitter `tags.scm` plugin pack. CodeNexus 路线: **吸收算法 + OSS 化 actionable 层 + 不做 GUI**, 跟 sentrux desktop-app 路线错开打.

## 2. 验证方法

直接拆 `github.com/sentrux/sentrux` (MIT 协议) 源码:

| 文件 | 大小 | 内容 |
|---|---|---|
| `sentrux-core/src/license.rs` | 16 KB | Ed25519 离线 license check |
| `sentrux-core/src/pro_registry.rs` | 4.8 KB | Pro feature 枚举 + dlopen 注册 |
| `sentrux-core/src/metrics/root_causes.rs` | 15 KB | **5 维评分公式 (核心 IP)** |
| `sentrux-core/src/metrics/dsm/mod.rs` | 21 KB | Design Structure Matrix |
| `sentrux-core/src/metrics/whatif/mod.rs` | 13 KB | What-if 算法 (OSS!) |
| `sentrux-core/src/metrics/evo/git_walker.rs` | 6.5 KB | Git history walker |

**全部 MIT 开源**. 算法层零黑盒.

## 3. Pro feature 拆解 (源 `pro_registry.rs`)

```rust
pub enum ProFeature {
    ExtraColorModes,    // 6 色模式: Age / Churn / Risk / Git / ExecDepth / BlastRadius
    FileDetailPanel,    // 函数级 metrics + imports + dependents
    EvolutionDetails,   // hotspot/coupling/bus_factor 详细
    WhatIfAnalysis,     // 修改预测
    McpDiagnostics,     // root_cause -> 文件名 actionable diagnostics
    UnlimitedRules,     // check_rules 数量限制解锁
}
```

注意: **算法实现 (whatif/, evo/, dsm/) 全在 OSS 里**, 运行时被 `pro_registry::has(...)` gating. 闭源 dylib 仅注册"开关", 算法 100% 公开.

## 4. 复刻难度评级

| Pro 功能 | 数据来源 | 算法依赖 | 复刻代码量 | 难度 |
|---|---|---|---|---|
| **McpDiagnostics** | DSM (free) + 邻接矩阵 | 简单图度数 | ~150 行 Python | **低** ✅ |
| **EvolutionDetails** | `git log --numstat` | trivial | ~80 行 | **极低** ✅ |
| **ExtraColorModes** | `os.stat` + `git log` + DSM 闭包 | 经典图遍历 | ~300 行 (5 个 mode) | **低** ✅ |
| **WhatIfAnalysis** | DSM 重建 + 重评分 | sentrux 已开源 | ~200 行 (调 sentrux scan 子进程) | **中** ⚠️ |
| **FileDetailPanel** | 多语言 AST | tree-sitter + tags.scm × 52 lang | ~5000 行 + 52 语言 plugin | **高** ❌ |
| **UnlimitedRules** | check_rules 配置 | 无 | 5 行 (改限制阈值) | **零** ✅ |

**5/6 复刻成本 < 1 天**. 唯一硬骨头是 FileDetailPanel 的 52 语言 plugin pack -- 这是 sentrux 唯一真护城河.

## 5. 5 维评分公式 (CodeNexus 直接抄)

源 `metrics/root_causes.rs`:

### Modularity (Newman 2004)
```
m = edge_count
intra = count edges where module(from) == module(to)
expected = Σ_module (Σ k_out × Σ k_in / m)
Q = (intra - expected) / m
modularity = (Q + 0.5) / 1.5  // 映射到 [0, 1]
```
Cite: Newman, M. E. J. (2004). "Finding and evaluating community structure in networks." Phys. Rev. E 69:026113.

### Acyclicity
```
acyclicity = 1 / (1 + cycle_count)
```

### Depth
```
depth = 1 / (1 + max_depth / 8)
```

### Equality (Gini 1912 on cyclomatic complexity)
```
sort cyclomatic_complexities ascending
G = Σ_i [(2(i+1) - n - 1) × CC_i] / (n × Σ CC)
equality = 1 - G
```
Cite: Gini, C. (1912). "Variabilità e mutabilità." Bologna.

### Redundancy
```
waste = (dead_count + duplicate_count) clamped to ≤ total
redundancy = 1 - waste / total
```

### 总分
```
quality_signal = (mod × acy × dep × equality × red)^(1/5) × 10000
```

**几何均值性质: 任一维度低必拖整体. 这是 sentrux UX 的核心驱动力 -- 不修最弱维永远过不了 gate.**

## 6. 实证: 在 mcm 上跑 replica

`D:/tmp/sentrux-replica.py` (200 行 Python, stdlib only) 跑出:

```
=== Modularity diagnostics for D:\projects\my-code-machine ===
files          : 55
import edges   : 77  (intra=13, cross=64)
packages       : 16

Newman Q       : +0.0477  (clamped [-0.5, 1.0])
normalized     : 0.3652  (3651 permille)

=== Top 10 modularity offenders ===
cross/out  blast   size  file
   1.00        2    1k  mmc.cleaner
   1.00        1   21k  mmc.cli           ← 真凶
   1.00        0    0k  mmc.__main__
   1.00        2    2k  mmc.commands.audit
   1.00        3    2k  mmc.commands.list_cmd
   ...
```

**actionable**: 21 KB 的 `mmc.cli` 是 cross-module 出度元凶, 拆它能跨档. (这正是本 session 早些时候我手工诊断 + skills cluster 提取后的方向.)

**Q 数值跟 sentrux 报的 0.0004 差 100 倍是因为 package 边界选择**: 我用 depth=2 (`mmc.skills`, `mmc.commands` 各为一个 module), sentrux 用 depth=1 (整个 mmc 一个 module). depth=1 时所有边都是 intra=expected, Q≈0. **我的 depth=2 反而更有 actionable 价值** -- 它告诉你"如果分子 package, modularity 已经是 D+ 了".

## 7. CodeNexus 设计建议

### 必做 (Phase 4)

1. **吸收 5 维公式**: 抄 sentrux `root_causes.rs` 算法, Apache 2.0 + paper cite. ~500 行 Rust. 直接给 CodeNexus 一个 `quality_signal` MCP tool, 跟 sentrux 输出可对比.
2. **OSS 化 McpDiagnostics**: sentrux 锁这层, CodeNexus 直接 OSS. 这是**最大差异化武器** -- 同一 quality 分数, 我们告诉用户该改哪个文件, sentrux 不告诉. 一键超车.
3. **Agent-first 输出格式**: sentrux Pro 卖 desktop treemap, CodeNexus 不做 GUI, 改做 LSP-style `file:line:reason` JSON. 给下游 agent (Claude/MiMo/Codex) 直接消费. 这是"给 agent 用的 sentrux".

### 可选 (Phase 5+)

4. **What-if analysis**: sentrux OSS 已有, CodeNexus 抄就行. 暴力 re-scan 兜底.
5. **Evolution details**: `git log --numstat` 30 行就够. 早做.
6. **Color modes** (Age/Churn/BlastRadius): 数据全标准 unix 工具. 可选输出维度.

### 不做 / 让 sentrux 卖

7. **Treemap renderer / desktop GUI**: 不在 CodeNexus scope. 用户要 viz 自己接 D3 或开 sentrux 免费版看图.
8. **52 语言 tree-sitter plugin pack**: sentrux 真护城河. CodeNexus 早期只支持 Python/TS/Rust 三个就够 (Curry 主栈), 不打全语言战.

## 8. 解锁 sentrux Pro 的合法路径 (备忘)

1. **(最干净) 抄进 CodeNexus**: 算法 cite Newman/Shannon/Gini/Kolmogorov 论文, 实现抄 sentrux MIT 源码, Apache 2.0 + NOTICE 标 attribution. **推荐**.
2. **(次净) Fork sentrux 自用**: clone → 改 `pro_registry.rs::has()` 永真. MIT 允许. 不能 redistribute 为 "sentrux" 商标. 仅 Curry 自己用 OK.
3. **(灰色) Hex-edit binary**: MIT 给了源就别动 binary. 不推荐.

## 9. 风险

- **CodeNexus 抄 sentrux 算法**, 可能撞 patent? 排查: Newman Q (公开论文), Gini (1912), Shannon (1948), Kolmogorov (1965) -- 全部超出 patent 期限. **零专利风险**.
- **Tree-sitter 多语言 plugin** 是个长尾工程. CodeNexus 早期不打全语言.
- **sentrux 升级新算法**: MIT 可继续抄. 跟踪 upstream.

## 10. 时间线建议 (CodeNexus)

| Phase | 内容 | 估时 |
|---|---|---|
| 4 | 吸收 5 维公式 + DSM + Diagnostics OSS | 3 天 |
| 5 | What-if + Evolution + Color modes | 2 天 |
| 6 | Python/TS/Rust tree-sitter integration | 1 周 |
| 7+ | 多语言扩展 (按需) | 长尾 |

## 11. 引用

源代码:
- https://github.com/sentrux/sentrux (MIT)
- https://raw.githubusercontent.com/sentrux/sentrux/main/sentrux-core/src/metrics/root_causes.rs
- https://raw.githubusercontent.com/sentrux/sentrux/main/sentrux-core/src/pro_registry.rs

学术:
- Newman, M. E. J. (2004). Phys. Rev. E 69:026113.
- Shannon, C. E. (1948). Bell System Tech. J. 27:379-423.
- Gini, C. (1912). Variabilità e mutabilità. Bologna.
- Kolmogorov, A. N. (1965). Probl. Inform. Transmission 1:1-7.

## 12. 实证产物

- `D:/tmp/sentrux-replica.py` -- 200 行 Python, McpDiagnostics + BlastRadius 复刻
- `~/.claude/projects/.../memory/reference_sentrux_internals.md` -- 完整 internals 备忘 (含公式 + license 机制 + Pro 注册表)
- mcm baseline: `quality_signal=6584` (post skills cluster), 5 维 raw 全部 captured for regression
