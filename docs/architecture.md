# Scheme → wasmtime:架构研究与设计文档

- **日期**:2026-07-02(所有网络事实均于当日获取)
- **方法**:30 个研究 agent 的多阶段 workflow(事实核查 → 对抗性交叉验证 → 逐项目调研 → Hoot/Steel 设计深挖 → 参考资料收集),每条关键论断标注来源与置信度;架构设计参考本地技能:`compiler-frontend`(前端/符号表)、`interpreters`(VM dispatch/值表示)、`jit-compilation`(Cranelift/JIT 远期路线)、`abi-and-calling-conventions`(AOT 调用约定)、`wasm-wasmtime`(wasmtime embedding/fuel/调试)。
- **置信度标记**:【高】= 一手来源直接验证且交叉核查一致;【中】= 一手来源但未双重验证,或来源略旧;【低】= 推断/未能直接验证。

---

## 1. 执行摘要

**推荐路线:可插拔双后端架构——核心交付一个 Rust 字节码 VM 解释器(MVP,6 周),wasm-GC AOT 后端作为架构预留的第二后端(第 7–8 周做最小 spike,之后长期深化)。**

四条理由:

1. **时间可行性**。Hoot 团队(Andy Wingo 级别的编译器作者)做 Scheme→wasm-GC AOT 用了 2023–2026 三年,其中 tailification(全程序 CPS 变换)、三显式栈、宏 phasing 等都是重难点。1–2 个月单人预算做 AOT-only 的可运行 MVP 不现实;字节码 VM(Steel、Crafting Interpreters 均为先例)6 周内可以做到"可运行、可展示、有测试与 benchmark"。
2. **新颖性在组合,不在单点**。"Rust 里的 Scheme 字节码 VM"已有 Steel(成熟)和 scheme-rs(活跃)两个先例,单做 VM 不新;"Scheme→wasm-GC"已有 Hoot。但 **"共享前端 + VM/AOT 双后端 + 生成的 wasm-GC 模块跑在 wasmtime 上(Rust host 提供运行时 imports)"这个组合没有任何项目做过**——Hoot 明确不支持 WASI/wasmtime(README 原话,见 §2),Wingo 的 Wastrel 跑 Hoot 输出是绕过 wasmtime 直接 wasm→C→native。这是真正的空白,而且 Wastrel 已经把"一个非浏览器 host 需要实现哪些 imports"探明了路线图(§9)。
3. **时机极好**。wasmtime 的 GC 默认开启(PR #13594,2026-06-08 合并)和异常处理 Tier-1 化(PR #13603,2026-06-09 合并)都发生在**一个月内**,将随 v47(约 2026 年 7 月)发布;WASI 0.3 于 2026-06-11 刚发布。现在入场做 "Scheme on wasmtime GC" 正处在生态窗口期。
4. **风险对冲**。VM 核心本身就是完整可交付的项目(对齐求职叙事:解释器工艺 + 性能 + DX);AOT spike 若超期,不影响 MVP 交付。反之 AOT-only 有实测性能风险:Wingo 2026-04-07 的基准显示 wasmtime 上 `return_call` dispatch 反而比 switch dispatch 慢(≈6.5x vs ≈4.3x 相对 native 开销),wasmtime 的 tail-call 代码生成尚不成熟。

**一句话版本**:先做一个测试驱动、benchmark 驱动的 Rust Scheme 字节码 VM(R7RS-small 子集),前端与 IR 从第一天按双后端设计;wasm-GC AOT 以"算术+闭包+尾调用子集跑通 wasmtime"为 spike 验收,作为长期差异化方向。

---

## 2. 关键事实核查(修正 7 天前的旧结论)

所有条目均为 2026-07-02 获取;"旧结论"指一周前初步调研的判断。

| # | 旧结论 | 核查结果 | 关键来源 | 置信度 |
|---|--------|----------|----------|--------|
| F1 | wasmtime 的 GC/tail-call/exceptions "已是 Tier-1 默认开启" | **部分错误,且一个月内剧变**。三者均 Tier-1,但:GC 在当前发布版 v46.0.1(2026-06-24)**仍需显式开启**(`Config::wasm_gc(true)` / CLI `-W gc`);默认开启的 PR #13594 已于 2026-06-08 合并进 main,随 v47(约 2026-07)发布,同时移除文档中 "not ready for primetime" 警告。tail-call 自 v21/v22(2024-05/06)默认开启。异常处理(`try_table`/`exnref`)2025-08 实现,但 **2026-06-09(PR #13603)才默认开启并升 Tier-1**,随 v46.0.0(2026-06-22)发布——成熟度只有一个月,不是一年 | [PR #13594](https://github.com/bytecodealliance/wasmtime/pull/13594)、[PR #13603]、[issue #5032](https://github.com/bytecodealliance/wasmtime/issues/5032)(2026-06-08 关闭)、[stability-wasm-proposals](https://docs.wasmtime.dev/stability-wasm-proposals.html)、release-46.0.0 分支 config.rs | 高 |
| F2 | wasmtime GC "lightly fuzzed 且有性能问题" | **"lightly fuzzed" 已被官方文档否定**:proposal 矩阵中 gc 行 Fuzzed ✅ 全勾。性能:实现自 v27.0(2024-11)完整;**默认收集器 2026-06-22(v46.0.0)刚从延迟引用计数(DRC)切换为新的 Copying 收集器**(可回收环、分配延迟更好,PR #13439);性能优化跟踪 issue #9351 已于 2026-05-26 关闭。注意:发布版 API 文档仍有自相矛盾的陈旧注释(`Collector::Copying` 的 rustdoc 还写着 "not yet functional") | [stability-wasm-proposals]、[Collector enum](https://docs.wasmtime.dev/api/wasmtime/enum.Collector.html)、[v45/v46 RELEASES]、[#9351] | 高 |
| F3 | (未覆盖)| **新发现的硬约束:GC/tail-call/exceptions 均 Cranelift-only**。Winch(基线编译器)在任何架构上都不支持 GC 和 tail-call(#9732、#9928 仍开放)。启用 `wasm_exceptions` 还要求编译时开 `gc` cargo feature | stability-tiers.html、config.rs `#[cfg(feature="gc")]` | 高 |
| F4 | "唯一真正用到 wasmtime GC 的路线是 AOT 发射 wasm-GC 类型的 .wasm" | **确认正确**。host-in-wasm 路线(解释器编译到 wasm32)的对象活在线性内存,由解释器自带 GC 管理,wasmtime 的 GC 完全看不见(Chibi/Ribbit/Otus/scheme.wasm/AssemblyScript 全部如此,AssemblyScript 自带 TLSF+增量 mark-sweep,明确不用 wasm-GC 提案) | 各项目调研(§3) | 高 |
| F5 | "Hoot 不支持 WASI/wasmtime,只支持浏览器和 Node 22+" | **结论确认,但原因需修正**。Hoot README 原话:"Hoot is currently unsupported on WASI runtimes";支持 Firefox 121+/Chrome 119+/Safari 26+/Node 22+。但阻塞原因**不是**"wasmtime 缺 GC"(那是过时认知)——真正阻塞是 Hoot 输出模块的 **JS 形状 host import ABI**(reflect.js:bignum 走 JS BigInt externref、WTF-8↔host 字符串转换、DWARF 反查 `code_name`/`code_source`、weak refs、异步等)且无 component-model 目标。维护者在 issue #882(2026-04-14)确认只有"长期计划"。**新事实**:Wingo 的 Wastrel(wasm→C→native AOT,自带 Whippet GC + WASI 0.1)已于 2026-04-09 实现 "full Hoot support"——但它绕过 wasmtime。**"Hoot 式模块跑在 wasmtime 上"仍无人做过** | [Hoot README(Codeberg)](https://codeberg.org/spritely/hoot)、[issue #882]、[wingolog 2026-04-09](https://wingolog.org/archives/2026/04/09/wastrel-milestone-full-hoot-support-with-generational-gc-as-a-treat) | 高 |
| F6 | (未覆盖)| **stringref 提案已死**:wasmtime 未实现、连跟踪 roadmap 都没有;Hoot 用后置 pass 把 stringref 降为 `(array i8)` WTF-8 + host 转换 imports。**不要在 stringref 上建任何东西** | stability-wasm-proposals.md 原文、[requiem for a stringref](https://wingolog.org/archives/2023/10/19/requiem-for-a-stringref) | 高 |
| F7 | (未覆盖)| **组件模型与 GC 不通**:GC 引用不能跨 component 边界(canonical ABI 仍是线性内存;#525 只是 pre-proposal,wasmtime #10325 自 2025-03 无任何实现动作)。WASI 0.3.0 已于 2026-06-11 发布并在 wasmtime 46 默认支持,但对 wasm-GC 语言运行时,**务实路径是 core module + 自定义 host imports,不做 component** | [component-model #525]、[Road to Component Model 1.0(BA,2026-06-08)](https://bytecodealliance.org/articles/the-road-to-component-model-1-0)、v46.0.0 release notes | 高 |
| F8 | 难度梯度:tail call < 数值塔 ≈ dynamic-wind < syntax-rules < 完整 call/cc | **按路线拆分后需修正**(§8):在**自管栈的字节码 VM**上,完整多发 call/cc 只是"惰性克隆帧栈",难度中等(Steel 已用 Open/Closed ContinuationMark 实现,可直接借鉴);在 **AOT/wasm** 上它才是最难的(需要 Hoot 式 tailification 全套)。dynamic-wind 在两条路线上都是 call/cc 之上的纯 Scheme 库层(winders 列表,Steel/Guile 同款)。escape-only 仍可作为快速路径先做 | Steel vm.rs 深挖、Hoot cps-in-hoot | 高 |
| F9 | Steel = 成熟 embeddable bytecode-VM Scheme,不做 wasm-GC AOT | **确认,并补充**:活跃至 2026-07-02 当天;MIT OR Apache-2.0;Helix 集成 PR #8675 **仍未合并**;有实验性 Cranelift JIT(半成品);另有新竞品 **scheme-rs**(maplant,R6RS,CPS→Cranelift JIT,tokio 异步优先,MPL-2.0,活跃但仅 26/2445 R6RS 测试通过)——同样无 wasm 目标 | §3 项目表 | 高 |

> 工具链补充【高】:`wasm-encoder` 0.252.0 完整支持 GC 类型/`return_call`/`try_table` 的编码;`wasmparser` 可按 feature 验证;`walrus`(已迁移到 wasm-bindgen org)的完整 GC 支持 **2026-03-25 才合并**(PR #304),视为"新且在演变",MVP 阶段不押注;Binaryen `wasm-opt` 有专门的 [GC Optimization Guidebook](https://github.com/WebAssembly/binaryen/wiki/GC-Optimization-Guidebook)(`--gufa`、`--closed-world`、type-ssa/merging),对 AOT 后端是重要的可选后处理器。

---

## 3. 同类项目全景

### 3.1 对比表(全部 2026-07-02 核实)

| 项目 | 技术路线 | 目标运行时 | wasm-GC? | 语言完整度 | 状态/许可证 | 对本项目的意义 |
|---|---|---|---|---|---|---|
| **Steel**(mattwparas)| Rust 字节码 VM(~140 opcodes,超指令);实验性 Cranelift JIT | native 嵌入、Helix 插件(PR 未合并)、playground(VM 编译到 wasm32)| 否 | R5RS 基本全(缺 let-syntax);多发 call/cc、syntax-rules/case、完整数值塔;R7RS 进行中;module 用 require/provide 非标准 | 活跃(当天有提交);MIT/Apache-2.0 | **VM 设计的头号参考**(§5、§8);同时是"纯 VM 不新颖"的证据 |
| **Guile Hoot**(Spritely)| AOT:Guile 前端(psyntax→Tree-IL→CPS soup)+ tailification → 自研 10k 行 wasm 工具链 → wasm-GC | 浏览器(FF121+/Chrome119+/Safari26+)、Node 22+;**明确不支持 WASI/wasmtime** | **是**(i31 fixnum、tag+hash 头 struct、三显式栈)| R7RS-small 良好;prompts 为原语,call/cc=默认 prompt;psyntax 全宏;bignum 走 host import;dynamic-wind 有已知 bug | 活跃(v0.9.0,2026-05-13;已迁 Codeberg);Apache-2.0 + 部分 LGPLv3+ | **AOT 后端的头号参考**(§9);其不做 wasmtime 正是本项目空白所在 |
| **Wastrel**(Wingo,新)| wasm→C→GCC native AOT,内嵌 Whippet GC,自实现 WASI 0.1 + Hoot 全部 host imports(mini-gmp bignum、WTF-8 转换、DWARF 反查)| native(绕过一切 wasm 运行时)| 消费 wasm-GC 输入 | n/a(不是 Scheme 实现,是 wasm 实现)| 活跃(2026-04-09 达成 full Hoot support)| **把"非浏览器 host 需要实现什么"探明的路线图**;也证明该空白有人在从另一侧逼近 |
| **Schism**(Google)| AOT:R6RS 子集→wasm(reference-types + tail-call 早期提案),**GC 靠 host JS 分配**,自举 | 需实验 flag 的 V8 | 否(前 GC 时代)| 极小子集:无宏/无 call/cc/无 record/int32-only | **已归档**(2021);Apache-2.0 | 历史先例;自举分阶段思路可借鉴;证明"最小可自举子集"的取舍 |
| **Chibi-Scheme** | C 解释器 → emscripten → 浏览器(host-in-wasm)| 浏览器;非 WASI/wasmtime | 否(自带 mark-sweep 于线性内存)| **R7RS-small 完整**+完整 call/cc+数值塔+20+ SRFI | 活跃(0.12,2026-03);BSD-3 | R7RS 语义参照实现;测试语料来源 |
| **Gambit** | AOT→C 为主;universal backend 出 JS/Python;emscripten 解释器 ~10MB | native/浏览器(JS)| 否 | R7RS 完整,全特性 | 活跃(v4.9.7,2025-07);Apache-2.0/LGPL | 多后端架构先例;Feeley 的 JS 后端论文 |
| **CHICKEN** | AOT→C,Cheney-on-the-MTA(栈即 nursery,CPS 全程)| native | 否 | R5RS 全,R7RS 进行中;call/cc 极廉价 | 活跃;BSD-3 | **AOT 流水线与续延/GC 一体化设计的经典教材**(非竞品)|
| **AssemblyScript** | TS 方言 AOT→wasm(Binaryen),**自带线性内存 GC**(TLSF+增量 mark-sweep,20 字节对象头)| 浏览器/Node/WASI 均可(环境无关)| **否**(明确未采用)| n/a(非 Scheme,无闭包/尾调用/宏)| 活跃;Apache-2.0 | "语言→wasm 工具链"工程参考:多运行时变体(incremental/minimal/stub)、transform hooks;同时证明线性内存 GC 路线与 wasm-GC 路线互斥 |
| **scheme-rs**(maplant)| Rust,CPS IR → Cranelift **JIT**;tokio 异步优先 | native 嵌入;无 wasm | 否 | R6RS 目标但早期(26/2445 测试);有 syntax-case、delimited conts、malachite bignum | 活跃(前一天有提交);MPL-2.0 | 第二个 Rust Scheme 竞品;其 JIT 路线与本项目 VM+AOT 路线互补不重叠 |
| **Ribbit** | 极小 R4RS VM(rib 三元胞),可输出 WAT(线性内存+自带复制 GC),**有 WASI 绑定** | 25 种宿主含 wasmtime | 否 | R4RS:有 call/cc/尾调用,无宏/record/异常/数值塔 | 活跃;BSD-3 | "极小 VM 上 wasmtime"的先例,但 R4RS+线性内存,与本项目定位不同 |
| **wasm-GC 语言工具链**(Kotlin/Wasm、dart2wasm、Scala.js-wasm、MoonBit、wasm_of_ocaml/Wasocaml)| 各自 IR→wasm-GC(struct/array + i31 unboxing 为共同模式)| 主要浏览器/Node;Kotlin 仅 WASI 0.1;dart 无 WASI;MoonBit 有 component-model 路径(其组件路线用线性内存 RC 后端) | 是(除 wasm_of_ocaml 用线性内存)| n/a | 均活跃 | 现代 wasm-GC 布局实践(i31 unboxing、Binaryen 后优化);**没有一个把 wasm-GC 输出作为 wasmtime 生产路径**,空白判断成立 |
| 小项目:scheme-to-wasm(玩具,2020 停)、Wisp(Zig/CL 风味,WASI,树遍历)、Otus Lisp(C VM→浏览器)、scheme.wasm(手写 WAT 解释器,2023 停)| 各异 | — | 全部否 | 子集 | 多数停滞 | 佐证:linear-memory 路线常见,wasm-GC AOT + Scheme 无人做 |

### 3.2 空白判定(任务一结论)

- **重复造轮子区**:纯 Rust Scheme VM(Steel/scheme-rs 已占)、host-in-wasm 解释器(Chibi/Ribbit/Otus 已占)、浏览器向 Scheme→wasm-GC(Hoot 已占,且质量极高)。
- **真空白区**【高置信】:①Scheme AOT 到 wasm-GC 并运行在 **wasmtime**(Rust host 实现运行时 imports:bignum、字符串转换、IO——Wastrel 已证明该 import 清单是有限且可实现的);②**同一前端下 VM 与 wasm-GC AOT 双后端可切换 + 差异测试**;③wasm 侧的 fuel metering / DWARF 调试 / 沙箱叙事(wasmtime 独有能力,浏览器路线没有)。
- **风口判断**:wasmtime GC 默认开启发生在过去 30 天内(F1),EH Tier-1 同样;生态刚从"不可行"翻到"可行",尚无 Scheme 项目跟进。

---

## 4. 路线对比与明确推荐

| 维度 | A:纯字节码 VM | B:纯 AOT→wasm-GC | C:可插拔双后端(推荐)|
|---|---|---|---|
| 新颖性 | 低(Steel/scheme-rs 在前)| 高(wasmtime 侧无人做)| **高**(组合无人做,且 VM 部分立即可用)|
| 1–2 月可交付性 | **高**(6 周核心)| **低**(tailification/对象表示/host ABI 三座山;Hoot 花了三年)| **高**(VM 为 MVP,AOT 为 spike)|
| 对作者 Rust 水平要求 | 中(enum+match VM,无 unsafe 硬需求)| 高(wasm-encoder 类型系统、GC 子类型、双运行时调试)| 中(spike 范围可控)|
| 展示/说服力 | 中(要靠工程质量与 perf 故事取胜)| 高但赌博(做不完=零)| **高**(VM 的工艺 + AOT 的前沿性叠加;差异测试本身就是亮点)|
| 性能故事 | 完全可控(dispatch/值表示/GC 逐步优化,benchmark 前后对比)| 受 wasmtime 摆布(tail-call codegen 尚不成熟,F1/Wingo 数据)| VM 故事托底,AOT 故事加分 |
| 失败模式 | 被"又一个玩具 Scheme"质疑 | 超期烂尾 | AOT spike 缩水为 demo,VM 仍完整 |

**明确推荐:路线 C。**并且明确内部优先级:**VM 是产品,AOT 是研究性差异化**。绝不允许 AOT 侵占前 6 周。

**远期 JIT 说明**(参考 `jit-compilation` 技能):若未来给 VM 加 JIT,应选 Cranelift(与 wasmtime 同一 codegen,叙事一致,scheme-rs 已示范可行性);Backend trait 天然容纳第三后端。明确不进 1–2 月范围。Steel 的教训(F9、§8):**不要让半接线的 JIT 脚手架躺在主 VM 文件里**——要么做完并 benchmark,要么不开工。

---

## 5. 总体架构与模块契约

前提:cargo workspace,单一共享前端,后端在 `Backend` trait 后可插拔。

```
                        ┌──────────────────────────────────────────────┐
 source text            │                 共享前端                      │
 ──────────► reader ────► expander(syntax-rules)────► CoreExpr(kernel)│
   (crate: reader)      │  (crate: expander)          (crate: core)    │
                        └───────────────┬──────────────────────────────┘
                                        │ trait Backend
                        ┌───────────────┴───────────────┐
                        ▼                               ▼
              bytecode compiler + VM          闭包转换 → (tailify?) → wasm-encoder
              (crate: vm)  【MVP】             (crate: backend-wasm)【stretch】
                        │                               │ .wasm (wasm-GC)
                        ▼                               ▼
                 native REPL / runner            wasmtime embedding host
                 (crate: cli)                    (crate: host)【stretch】
```

### reader —— datum 层,无宏无求值(技能:`compiler-frontend`)

```rust
pub fn read_all(src: &str, file: FileId) -> Result<Vec<Spanned<Datum>>, ReadError>;

pub enum Datum {
    Bool(bool), Fixnum(i64), Flonum(f64), Char(char),
    Str(String), Symbol(Sym),                    // Sym = interned
    Pair(Box<Spanned<Datum>>, Box<Spanned<Datum>>), Nil,
    Vector(Vec<Spanned<Datum>>), Bytevector(Vec<u8>),
}
```

- 每个节点带 `Span`,错误报告从第一天有位置信息(后续 DWARF 生成也依赖它)。
- 手写词法器(R7RS 词法不适合正则表);TDD 形态:`read_all("(a . b)")` 断言结构;非法输入断言带位置的错误。
- MVP 词法子集:`#|...|#`、`#;`、字符名、`#b/#o/#x`、字符串转义;datum label(`#0=`)后置。

### expander —— datum → CoreExpr,吸收一切派生形式与宏

- **内核形式只有 ~8 种**:`quote` `if` `set!` `lambda` `letrec*` `begin` `call` + 顶层 `define`。`let/let*/cond/case/and/or/when/unless/do/quasiquote` 全部为(内置或用户)宏。加语言特性大多只动这一层。
- **卫生策略【重要决策,证据来自 Steel 深挖】**:MVP 用**重命名式卫生**(宏模板引入的、非模式变量的标识符统一改名),而非 Racket 式 sets-of-scopes。Steel 用同款方案通过了真实 syntax-rules 测试集,实现成本低一个数量级。`Syntax` 对象接口上预留 scope-set 字段(MVP 恒空),给未来 syntax-case 升级留缝。
- **教训预埋(Hoot,wingolog 2024-01-05)**:全程序编译与宏 phasing 冲突——将来做 define-library 时必须"逐模块展开、残余化无宏 AST、再拼接",不能把所有模块 splat 进一个大 letrec。MVP 单文件不触发此问题,但 expander 的 API 按"模块为单位展开"设计。

### core —— 内核 IR + 后端无关 pass

```rust
pub enum CoreExpr {
    Const(Const), Ref(VarId), SetBang(VarId, Box<CoreExpr>),
    If(Box<CoreExpr>, Box<CoreExpr>, Box<CoreExpr>),
    Lambda { params: Vec<VarId>, rest: Option<VarId>, body: Box<CoreExpr> },
    Call { head: Box<CoreExpr>, args: Vec<CoreExpr>, tail: bool },
    LetRec { bindings: Vec<(VarId, CoreExpr)>, body: Box<CoreExpr> },
    Seq(Vec<CoreExpr>),
    PrimCall(Prim, Vec<CoreExpr>),
}
```

三个共享 pass,均可独立单测:
1. **尾位置标注**(填 `Call::tail`)——两后端的 TCO 都吃它;
2. **自由变量分析**——VM 闭包捕获与 AOT 闭包转换都吃它;
3. **assignment conversion**(被 `set!` 且被捕获的变量装箱)——Steel 与 Hoot 都有等价物。

VarId 由 expander alpha-renaming 保证唯一,后端不需要符号表(scoped hash map 只活在 expander 内部)。

**刻意不做的事**:CPS/ANF **不进共享 IR**。VM 不需要它;AOT 的 tailification(如果做完整续延)是 backend-wasm 的内部 pass。这是"续延不污染核心架构"的关键决策(Hoot 也是把 tailify 放在整个管线最末端)。

### backend trait —— 可插拔边界

```rust
pub trait Backend {
    type Artifact;
    fn compile_program(&mut self, prog: &Program) -> Result<Self::Artifact, CompileError>;
}
// vm:   Artifact = Chunk(字节码+常量池)→ 进程内执行
// wasm: Artifact = Vec<u8>(.wasm 模块)→ host crate 用 wasmtime 执行
```

- trait 只管"编译",不管"执行"(两个运行时差异太大,硬统一是坏抽象);cli 层用 `enum Engine` 汇合。
- **差异测试是双后端架构的最大红利**:同一份测试语料喂两个后端,输出必须一致。语料来源:自写 + Chibi(R7RS 参照实现)的行为 + ecraven/r7rs-benchmarks 子集。

### vm ——(技能:`interpreters`;设计证据:Steel 深挖)

| 决策点 | MVP 选择 | 理由与参考 |
|---|---|---|
| 机器模型 | 栈式字节码 | 编译器最简单;与 wasm 心智一致;Lua 式寄存器机列为 perf 里程碑(有基线后再评估)|
| dispatch | `loop + match u8` | 稳定版 Rust 无 computed goto;Steel 同款且够用。**注意 opcode 纪律**:Steel 的 140 个 opcode + 即兴超指令是其深挖里明确的反面教训——MVP 控制在 ~40 个以内,超指令等 benchmark 说话 |
| 值表示 | `enum Value`(fixnum i64/flonum f64 unboxed,堆对象走句柄)| NaN-boxing 列为 perf 里程碑,做前后 benchmark 对比(这本身是 README 素材)|
| 帧与栈 | VM 自管 `Vec<Frame>` + 操作数栈,**不用 Rust 原生调用栈递归** | 由此 TCO=复用帧(`TailCall` opcode,Steel `handle_tail_call` 同款);**完整多发 call/cc=惰性克隆帧栈**(Steel `ContinuationMark::Open→Closed` 方案可直接借鉴:捕获时先存弱引用,帧将被破坏时才提升为完整快照)|
| 内存管理 | 第一阶段 `Rc` + 已知 letrec 循环泄漏(文档明示);第 6 周起采用 **Steel 的混合方案**:不可变持久结构全走 Rc,仅对"可变盒"(被 set! 捕获的变量、可变 vector)加一层小型 mark-sweep | Steel 文档化的架构,兼顾实现量与循环回收;`Trace` trait 从第一天预留 |
| 已证明的优化(排期在 perf 里程碑)| last-use/move 分析(把变量最后一次使用从栈上 move 而非 clone,使 Rc 计数降到 1、持久结构可原地改)| Steel 深挖中"最有效的单项优化"(naive reverse 123ms → 23ms)|

### backend-wasm + host ——(stretch;§9 详述;技能:`wasm-wasmtime`、`abi-and-calling-conventions`)

- 发射用 `wasm-encoder`(0.252,GC/return_call/try_table 全支持【高】);**不用 walrus**(GC 支持 2026-03-25 才合并,churn 中);`wasm-opt` 作可选后处理。
- host 用 wasmtime crate(pin v46+,显式 `Config::wasm_gc(true)`、`wasm_tail_call(true)`、`wasm_exceptions(true)`;v47 起默认);**只用 Cranelift**(Winch 不支持 GC,F3)。
- 调试回路:`wasmprinter` golden-WAT 快照测试 → `wasmparser` 验证 → wasmtime 执行;`wasmtime explore` 与 DWARF(`Config::debug_info(true)`)后置。
- fuel metering 做 sandbox 演示(wasmtime 独有卖点,浏览器路线给不了)。

### cli —— REPL(rustyline)、runner、`--backend vm|wasm`、测试语料 runner、bench runner(hyperfine,Steel 同款做法)。

---

## 6. 可插拔/可扩展性落点

1. **前端喂多后端**:CoreExpr 是唯一交接面,零共享代码地被两个后端消费;第三后端(Cranelift JIT)只需再实现一次 trait。
2. **加特性的改动半径**:
   - 语法糖(when/case/quasiquote…)→ 只加宏定义;
   - 新数据类型(record/bytevector)→ runtime 值 + 少量 prim,CoreExpr 不动;
   - 控制流特性(call/cc、dynamic-wind)→ VM 加 opcode/原语,AOT 才需要 IR 级改造(tailify 作为 backend-wasm 内部 pass 增量插入);
   - 数值塔加厚(bignum)→ 只动 runtime Number 与算术 prim;AOT 侧走 host import(Hoot/Wastrel 先例,§9)。
3. **架构上预留、明确不做**:component model 暴露(GC 引用过不了 canonical ABI,F7,核心模块 + host imports 即正解);stack-switching 提案(Hoot 有实验分支,观望);多线程 wasm-GC(提案未覆盖)。

---

## 7. MVP 边界与周计划(6 周核心 + 2 周弹性)

**MVP 必须有**:reader(R7RS 词法子集)/expander(内核形式 + 基础 syntax-rules + quasiquote)/CoreExpr 三 pass/栈式 VM(闭包、TCO、错误子集)/fixnum+flonum 算术/pair、vector、string、symbol、char、bool、bytevector/define-record-type/多返回值/raise+guard 子集/REPL + runner/差异测试语料 + ecraven 子集 benchmark + CI。

**架构预留但 MVP 不做**:完整 call/cc、dynamic-wind、bignum/rational、define-library、syntax-case、NaN-boxing、mark-sweep GC、AOT 后端(骨架 crate + `(+ 1 2)` 级 spike 除外)、JIT。

| 周 | 交付 |
|---|---|
| W1 | workspace 脚手架 + CI;reader 全绿(TDD);值表示与 symbol interning |
| W2 | expander(内核形式)+ CoreExpr + 三 pass(各自独立测试)|
| W3 | 字节码编译器 + VM 核心:闭包/TCO/算术;REPL 跑通 |
| W4 | syntax-rules(基础省略号)+ quasiquote + record + 多返回值 |
| W5 | 错误处理子集 + 标准库(list/string/vector 常用面)+ 差异测试语料接入 |
| W6 | benchmark(ecraven 子集,hyperfine)+ 文档(README/设计文档/ADR)+ demo 打磨 |
| W7–8(弹性,按叙事价值排序)| **首选:wasm-GC AOT spike**(§9 验收标准);次选:完整 call/cc + dynamic-wind;三选:混合 GC 或 NaN-boxing(带前后对比数据)|

W7–8 首选 AOT spike 的理由:时机(F1 的 30 天窗口)+ 差异化(§3.2)+ 它只需要 CoreExpr 的一个子集(算术/闭包/尾调用,无续延),风险可控。

---

## 8. R7RS 特性支持矩阵(任务二 §4)

### 8.1 标准选型:为什么以 R7RS-small 为参照系(而非 R5RS)

脉络:R5RS(1998)→ R6RS(2007,大幅膨胀、社区分裂,多数小实现拒绝跟进)→ R7RS 拆成两半:**R7RS-small**(2013,回归 R5RS 的小语言精神)与 R7RS-large(至今以 SRFI 为单位缓慢推进,无人完整实现)。所以选型不是"小语言 vs 大语言",而是**同一个小语言的 1998 版和 2013 版**——R7RS-small 本质上是补了模块/异常/record 的现代化 R5RS,MVP 工作量与"R5RS 子集"相同。

选 R7RS-small 的四个目的:
1. **测试与对照生态全在这边**:差异测试的语义 oracle(Chibi,R7RS 参照实现)与 benchmark 套件(ecraven/r7rs-benchmarks)都是 R7RS 口径(§5 backend trait、§10);
2. **可比性**:§3 对比表以 R7RS 覆盖度为量尺(Hoot 目标 R7RS-small、Steel 的 R7RS "进行中"、scheme-rs 走 R6RS),且"标准 `define-library` 而非 Steel 式 require/provide 方言"是明确的差异化点;
3. **求职信号**:嵌入式 DSL 场景恰好用得上 R7RS 新增件(模块隔离、异常、record、`parameterize` 动态配置绑定);
4. **避免返工**:先按 R5RS 做,大小写敏感性、异常、模块日后全是破坏性迁移。

对本项目有实际影响的 R5RS → R7RS-small 增量:

| R7RS-small 新增/变化 | R5RS 的状态 | 对本项目的意义 |
|---|---|---|
| `define-library` 模块系统 | **完全没有模块概念** | 特性矩阵的模块行;没有它就只能像 Steel 那样发明 `require/provide` 方言(§3 中列为其减分项)|
| 异常系统:`raise`/`guard`/`with-exception-handler`/error 对象 | 没有(连 `error` 都非标准)| MVP 错误处理子集的标准语义依据;AOT 侧恰好映射到刚 Tier-1 的 `try_table`/`exnref`(F1)|
| `define-record-type` | 没有(靠 SRFI-9)| MVP 特性,已进矩阵 |
| bytevector 及其 IO | 没有 | MVP 特性;AOT 侧与 `(array i8)` 字符串同表示(§9)|
| `parameterize`/parameter 对象、`case-lambda`、`when/unless`、`let-values`、`define-values` | 没有 | 多数是 expander 层的廉价宏,近乎白送 |
| `cond-expand`、`include` | 没有 | 双后端条件编译正好用得上(Chibi 用它区分 emscripten/wasm32 平台,§3 点名值得借鉴)|
| **大小写敏感** | 大小写**不**敏感 | reader 第一天就要定的行为;按 R7RS 走避免日后破坏性迁移 |
| 词法扩展:`#\|...\|#` 块注释、`#;` datum 注释、`#true/#false` | 没有 | 已列入 reader 的 MVP 词法子集(§5)|
| 语义澄清:`letrec*`、内部 define 顺序、`equal?` 对循环结构必须终止、尾调用位置精确枚举 | 含糊或未定义 | CoreExpr 以 `letrec*` 为内核形式、尾位置标注 pass 直接引用 R7RS 定义,省去自行裁决语义 |

R5RS 已有、R7RS-small 原样保留的:完整 call/cc、`dynamic-wind`、`values`/`call-with-values`(这两个正是 R5RS 相对 R4RS 的增量)、数值塔结构、只含 `syntax-rules` 的卫生宏(`syntax-case` 属 R6RS,R7RS-small 刻意未收)。矩阵里最难的几行,两个标准之间没有难度差。

**定位声明**:目标不是完整实现 R7RS-small(那是 Chibi 花多年做的事),而是**以 R7RS-small 为参照系划一条有原则的子集线**——下表每行的"进/不进 MVP"都相对该标准而言,比自造无坐标的方言可辩护得多。

### 8.2 特性矩阵

难度分【VM】/【AOT】两列——这是对旧"难度梯度"的核心修正(F8):**同一特性在两条路线上的难度可以完全不同**。

| 特性 | MVP? | 难度 VM | 难度 AOT | 前置依赖 | 参考实现(经调研核实)|
|---|---|---|---|---|---|
| proper tail call | ✅ | 低(TailCall opcode 复用帧)| 低-中(`return_call`,但注意 wasmtime dispatch 性能数据,F1)| 尾位置标注 pass | Steel `handle_tail_call`;Hoot 原生 return_call;Clinger 1998 |
| call/cc(escape-only)| 预留(W7 候选)| 低-中(记录栈深 + 截断)| 中(exnref 展开可模拟 escape)| VM 自管栈 | CHICKEN(escape 快路径);wasmtime EH 刚 Tier-1 |
| call/cc(完整多发)| 预留 | **中**(惰性克隆帧栈;已有可抄方案)| **高**(需 tailification + 显式栈全套)| 同上;AOT 需 CPS 化 | **Steel** `ContinuationMark Open/Closed`(VM);**Hoot** tailify + 三栈(AOT);Hieb/Dybvig/Bruggeman 1990 |
| dynamic-wind | ❌(W7 随 call/cc)| 中(纯 Scheme 层 winders 列表)| 中(同左,但依赖续延正确性)| call/cc | **Steel** `parameters.scm` 全套可参考;Hoot 有 partial-evaluator bug 的教训 |
| syntax-rules | ✅(基础)| 中(重命名式卫生)| 同左(前端共享)| expander | **Steel** rename_idents(实践证明够用);Clinger/Rees《Macros That Work》 |
| syntax-case | ❌ | 高(需 psyntax 级投入或 sets-of-scopes)| 同左 | Syntax 对象升级 | Hoot 直接移植 Guile psyntax;Flatt《Binding as Sets of Scopes》 |
| 数值塔:fixnum+flonum | ✅ | 低 | 低-中(i31 打包 30 位 fixnum + flonum 装箱 struct)| — | Hoot(i31 + `(struct i32 f64)`)|
| 数值塔:bignum/rational | ❌(feature 预留)| 中低(`num-bigint`/`num-rational`,溢出检查后提升)| 中(**host import 提供 bignum 算术**,Rust host 用 num-bigint 实现)| 算术 prim 分层 | Steel(num-bigint 同款);**Hoot/Wastrel**(浏览器 BigInt / mini-gmp 的 import ABI 先例);scheme-rs 用 malachite |
| 多返回值 | ✅ | 低-中(栈上天然)| 中(wasm 多值返回 or values 对象)| — | R7RS `values`;Hoot 续延 arity 处理有已知 bug 的教训 |
| quasiquote | ✅ | 低(展开期重写)| 同左 | expander | 任意实现 |
| bytevector | ✅(基本)| 低 | 低(`(array i8)`)| — | R7RS;Hoot 字符串同表示 |
| define-record-type | ✅ | 低(展开为 prim + RecordType 值)| 中(struct 子类型 + tag)| record 值类型 | Hoot(带继承/opaque 扩展,文档好)|
| define-library | ❌ | 中-高 | 中-高(+宏 phasing 陷阱)| expander 按模块展开 | **Hoot 的教训必读**(wingolog 2024-01-05);Steel 偏离标准(require/provide)——本项目应走 R7RS 正统以示区分 |
| 异常(raise/guard)| ✅(子集)| 中(帧展开;完整语义与 dynamic-wind 交互后置)| 中(`try_table`/`exnref`,刚 Tier-1——注意只有一个月成熟度,F1)| 错误值类型 | R7RS/SRFI-34;Hoot 0.9 刚切标准 EH;wingolog "nominal types"(exnref 兼作名义类型)|
| 字符串(不可变起步)| ✅ | 低 | 低(`(array i8)` UTF-8/WTF-8;**永不依赖 stringref**,F6)| — | Steel(`Gc<String>` 不可变);Hoot(WTF-8 lowering)|
| eval / 交互顶层 | REPL ✅(编译到 chunk 逐条执行)| 低-中 | ❌(AOT 侧不做;Hoot 的 REPL 要 6.6MB 关掉 tree-shaking 的教训)| — | Hoot `-fruntime-modules` 尺寸数据 |

---

## 9. AOT 后端设计要点(源自 Hoot/Wastrel 深挖,均【高】置信一手来源)

Spike 范围(W7–8)与长期路线共用这些决策:

**对象表示**(wingolog "a world to win",2023-03-20):
- 统一类型 = `(ref eq)`(`eq?` 即 `ref.eq`);fixnum/char/bool 打包进 `(ref i31)`(30 位)。
- 堆对象共同基类 `struct $heap-object { i32 tag+hash }`,具体类型继承之(`$pair = (sub $heap-object (struct i32 (ref eq) (ref eq)))`)。**必须自带 tag 字**:wasm-GC 只有结构等价,`ref.test` 分不开两个同形状类型(Hoot 的核心教训之一)。
- 派发模式:`ref.test → ref.cast → tag 比较 → return_call` 精确类型内部函数。

**调用约定**(技能 `abi-and-calling-conventions` 的思路映射到 wasm 层):
- Spike 用最简形态:统一签名 `(func (param $closure (ref $closure)) (param $args (ref $argv)) (result (ref eq)))`,闭包 struct 携带 funcref + captures,天然支持 variadic/apply。
- Hoot 的进阶版(全函数 `(func (param i32))` + `$arg0..3` 全局"寄存器" + 溢出数组,模拟 x86-64 传参)列为优化项;注意 Wingo 实测现代 wasm 引擎 ABI 只有 6–8 个有效"寄存器位"。
- 不做 tailification(spike 无续延);`return_call` 直接覆盖尾调用。

**Rust host 必须提供的 imports(= Wastrel 探明的清单)**:bignum 算术(用 num-bigint 实现,对应 Hoot 的 BigInt/mini-gmp)、WTF-8↔host 字符串转换、float→string(Hoot 已把它移回纯 Scheme,可跟进)、`code_name`/`code_source` 反查(DWARF,后置)、weak refs(连 Wastrel 都还 stub 着,长期)、异步等待(JSPI 等价物,长期)。Spike 只需要前两项的最小面。

**验收标准(spike)**:`fib`、`(let loop ...)` 百万次迭代不涨栈、闭包捕获、字符串常量打印——四者经 `wasm-tools validate --features gc` 验证并在 wasmtime(`-W gc,tail-call,exceptions`)跑通;VM 与 AOT 对同一语料输出一致。

**性能预期管理**(F1/Wingo 2026-04 数据):wasmtime 上 wasm 代码 ≈ native 的 30–40% 慢(coremark);tail-call codegen 尚不成熟。**卖点定位在"沙箱 + 单一工具链 + 可插拔",不是"比 native VM 快"**。

---

## 10. 参考资料清单(分类,均验证可达,2026-07-02)

### 规范 / 一手资料
- WASM GC 提案 [MVP.md](https://github.com/WebAssembly/gc/blob/main/proposals/gc/MVP.md)、[Post-MVP.md](https://github.com/WebAssembly/gc/blob/main/proposals/gc/Post-MVP.md);[function-references](https://webassembly.github.io/function-references/core);[tail-call](https://webassembly.github.io/tail-call/core/);[exception-handling](https://webassembly.github.io/exception-handling/)
- [R7RS-small PDF](https://small.r7rs.org/attachment/r7rs.pdf);[WASI](https://github.com/WebAssembly/WASI) / [wasi.dev](https://wasi.dev)
- wasmtime:[proposal 支持矩阵](https://docs.wasmtime.dev/stability-wasm-proposals.html)、[Rust embedding API](https://docs.wasmtime.dev/api/wasmtime/)、[DWARF 调试](https://docs.wasmtime.dev/examples-debugging.html)、[fuel/epoch 中断](https://docs.wasmtime.dev/examples-interrupting-wasm.html)、[CLI](https://docs.wasmtime.dev/cli-options.html)、[内部架构](https://docs.wasmtime.dev/contributing-architecture.html)
- crates:[wasmtime](https://docs.rs/wasmtime)、[wasmtime-wasi](https://docs.rs/wasmtime-wasi)、[wasm-encoder](https://docs.rs/wasm-encoder)、[wasmparser](https://docs.rs/wasmparser)、[wat](https://docs.rs/wat)、[wasmprinter](https://docs.rs/wasmprinter)

### 编译器理论
- Appel《Compiling with Continuations》;Flanagan et al.[《The Essence of Compiling with Continuations》](https://dl.acm.org/doi/10.1145/155090.155113)(ANF);Kennedy [《Compiling with Continuations, Continued》](https://www.microsoft.com/en-us/research/wp-content/uploads/2007/10/compilingwithcontinuationscontinued.pdf)
- Matt Might:[closure conversion](https://matt.might.net/articles/closure-conversion/)、[CPS conversion](https://matt.might.net/articles/cps-conversion/)
- Ghuloum [《An Incremental Approach to Compiler Construction》](http://scheme2006.cs.uchicago.edu/11-ghuloum.pdf);Dybvig [《Three Implementation Models for Scheme》](https://www.cs.unm.edu/~williams/cs491/three-imp.pdf);Hieb/Dybvig/Bruggeman [《Representing Control…》](https://www.cs.tufts.edu/~nr/cs257/archive/kent-dybvig/stack.pdf);Clinger [《Proper Tail Recursion and Space Efficiency》](https://www.cs.tufts.edu/~nr/cs257/archive/will-clinger/proper-tail-space.pdf)
- 卫生宏:Kohlbecker et al. 1986;[Clinger/Rees《Macros That Work》](https://xivilization.net/~marek/tex/hellprog/papers/p155-clinger.pdf);[Flatt《Binding as Sets of Scopes》](https://con.racket-lang.org/2015/flatt.pdf)
- 书:Queinnec[《Lisp in Small Pieces》](https://christian.queinnec.org/WWW/LiSP.html);[Crafting Interpreters](https://www.craftinginterpreters.com/)
- 基准:[ecraven/r7rs-benchmarks](https://github.com/ecraven/r7rs-benchmarks) + [结果面板](https://ecraven.github.io/r7rs-benchmarks/)

### 同类实现设计文章(wingolog Hoot 系列为主,按时间)
- 2023-03-20 [a world to win](https://wingolog.org/archives/2023/03/20/a-world-to-win-webassembly-for-the-rest-of-us)(对象表示/传参/prompts/数值塔——**AOT 设计的单篇最佳入口**)
- 2023-10-19 [requiem for a stringref](https://wingolog.org/archives/2023/10/19/requiem-for-a-stringref);2023-11-24 [tree-shaking](https://wingolog.org/archives/2023/11/24/tree-shaking-the-horticulturally-misguided-algorithm)
- 2024-01-05 [scheme modules vs whole-program compilation](https://wingolog.org/archives/2024/01/05/scheme-modules-vs-whole-program-compilation-fight)(宏 phasing 陷阱)
- 2024-05-16 [on hoot, on boot](https://wingolog.org/archives/2024/05/16/on-hoot-on-boot);2024-05-22 [growing a bootie](https://wingolog.org/archives/2024/05/22/growing-a-bootie);2024-05-24 [hoot's wasm toolkit](https://wingolog.org/archives/2024/05/24/hoots-wasm-toolkit);2024-05-27 [cps in hoot](https://wingolog.org/archives/2024/05/27/cps-in-hoot)(tailification 详解)
- 2025-10-30 [wastrel](https://wingolog.org/archives/2025/10/30/wastrel-a-profligate-implementation-of-webassembly);2026-02-06 [AOT wasm-gc in wastrel](https://wingolog.org/archives/2026/02/06/ahead-of-time-wasm-gc-in-wastrel);2026-02-18 [two mechanisms for dynamic type checks](https://wingolog.org/archives/2026/02/18/two-mechanisms-for-dynamic-type-checks);2026-03-10 [nominal types in webassembly](https://wingolog.org/archives/2026/03/10/nominal-types-in-webassembly);2026-03-31 [wastrelly wabbits](https://wingolog.org/archives/2026/03/31/wastrelly-wabbits)(host import 清单);2026-04-07 [the value of a performance oracle](https://wingolog.org/archives/2026/04/07/the-value-of-a-performance-oracle)(**wasmtime dispatch 基准,必读**);2026-04-09 [wastrel milestone](https://wingolog.org/archives/2026/04/09/wastrel-milestone-full-hoot-support-with-generational-gc-as-a-treat)
- Spritely:[Hoot 首页](https://spritely.institute/hoot/)、[0.9.0 release](https://spritely.institute/news/hoot-0-9-0-released.html)、[v0.4.0 release](https://spritely.institute/news/guile-hoot-v0-4-0-released.html)(dynamic-wind bug、call/cc=默认 prompt);dthompson [Wasm GC isn't ready for realtime graphics](https://dthompson.us/posts/wasm-gc-isnt-ready-for-realtime-graphics.html)(2025-01,GC 性能反例)
- Bytecode Alliance:[Wasmtime 27.0(GC 完成)](https://bytecodealliance.org/articles/wasmtime-27.0)、[Road to Component Model 1.0](https://bytecodealliance.org/articles/the-road-to-component-model-1-0)(2026-06-08)、[WASI 0.3](https://bytecodealliance.org/articles/WASI-0.3)(2026-06-11)、fitzgen [New Stack Maps](https://bytecodealliance.org/articles/new-stack-maps-for-wasmtime)、[Security and Correctness in Wasmtime](https://fitzgen.com/2022/09/13/security-and-correctness-in-wasmtime.html)
- Scheme Workshop 2024 [Scheme on WebAssembly: It is happening!](https://icfp24.sigplan.org/details/scheme-2024-papers/6/Scheme-on-WebAssembly-It-is-happening-)(Wingo keynote);FOSDEM 2024/2025/2026 相关 talk(见正文链接)

### 工具链参考
- [wasm-tools 仓库](https://github.com/bytecodealliance/wasm-tools)(encoder/parser/printer/wast 一体);[Binaryen GC Optimization Guidebook](https://github.com/WebAssembly/binaryen/wiki/GC-Optimization-Guidebook)(`--gufa`/`--closed-world`/type-* passes)
- [walrus(wasm-bindgen org)](https://github.com/wasm-bindgen/walrus)——GC 支持 2026-03-25 合并,观察名单
- 竞品源码作教材:[Steel](https://github.com/mattwparas/steel)(vm.rs 的 continuation/尾调用、`parameters.scm` 的 dynamic-wind、passes/analysis.rs 的 move 优化)、[Hoot](https://codeberg.org/spritely/hoot)(tailify.scm、lib/hoot/)、[scheme-rs](https://github.com/maplant/scheme-rs)、[Wastrel](https://codeberg.org/andywingo/wastrel)

---

## 11. 开放问题(作者决策清单)

1. **项目名与 crate 前缀**——越早定越好,影响全部代码与 README 叙事。
2. **许可证**——建议 MIT OR Apache-2.0 双许可(Steel 同款,Rust 生态默认;避免 Hoot 式混合 LGPL 的复杂度)。
3. **wasmtime 版本策略**——pin v46 显式开 GC/EH,还是等 v47(默认开)再做 spike?建议:不等,`Config` 显式设置对两个版本都对。
4. **字符串语义**——R7RS 可变字符串 vs 不可变 + `string-copy`?(Steel 选了不可变;AOT 侧 `(array i8)` 天然偏不可变;倾向不可变,但这偏离 R7RS,需要明示)
5. **W7–8 弹性周的最终取舍**——AOT spike(差异化)vs 完整 call/cc(R7RS 正统)vs GC/NaN-boxing(性能叙事)。本文推荐 AOT spike,但如果目标岗位更看重解释器深度,call/cc + dynamic-wind 组合(Steel 方案可抄,约一周)可提前。
6. **对 Steel/scheme-rs 的公开定位**——README 里如何诚实叙述与两者的关系(建议:明确"互补:它们做嵌入,本项目做 wasm-GC 编译目标 + wasmtime 宿主")。
7. **bignum host-import ABI**——AOT 侧 bignum 走 import 时的具体签名设计(参考 Wastrel 对 Hoot import 集的实现;需在 spike 之后、bignum 之前定)。
8. **基准口径**——对 Steel/Chibi 跑同套 r7rs-benchmarks 子集并公开数字?(诚实的对比表说服力最强,但要预算调参时间)

---

## 附录:本次调研的已知局限

- 个别 haiku 级调研 agent 的原始输出含有错误(如"wasmtime 不支持 wasm-GC"的过时说法、把 Codeberg star 数误作 commit 数),均已被 sonnet 对抗核查纠正;正文只采用核查后的结论。
- Hoot 的 stringref-lowering 细节主要来自 2023 年博文,0.9.0 时代未找到一手再确认(标【中】);Deno 支持细节来自二手来源。
- 所有 wasmtime "即将默认开启"的表述基于 main 分支已合并 PR 与月度发布节奏推断 v47 时间,存在 ±1 个发布周期的不确定性。
