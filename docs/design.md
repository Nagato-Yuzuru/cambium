# Cambium workspace 设计:crate 边界与职责

- **日期**:2026-07-03(依赖版本与竞品结构均于当日实查;调研方法见附录)
- **上游文档**:[`architecture.md`](architecture.md) 是调研与路线决策的权威记录(为什么做双后端、R7RS-small 子集线、AOT 设计要点)。本文档不重复它,只做工程落地:workspace 布局、每个 crate 的边界/职责/公开 API、第三方依赖选型、按 crate 的工作拆分。两文冲突时,路线问题以 architecture.md 为准,工程细节以本文为准。
- **现状基线**:workspace `members = []`,还没有任何 crate。W1 实际只交付了工具链脚手架(mise/just/prek/CI/deny);architecture.md §7 计划中 W1 的 reader 部分未开工,计入 §6 的工作拆分。

---

## 1. Workspace 总览

### 1.1 Crate 清单

| #   | crate                  | 类型 | 阶段        | 一句话职责                                                                                |
| --- | ---------------------- | ---- | ----------- | ----------------------------------------------------------------------------------------- |
| 1   | `cambium-common`       | lib  | MVP         | Span/FileId/Spanned、Symbol interning(含 gensym)、SourceMap、诊断数据模型                 |
| 2   | `cambium-reader`       | lib  | MVP         | 源文本 → `Vec<Spanned<Datum>>`,R7RS 词法子集,无宏无求值                                   |
| 3   | `cambium-core`         | lib  | MVP         | 内核 IR(CoreExpr/Const/Prim)、三个共享 pass、`Backend` trait                              |
| 4   | `cambium-expander`     | lib  | MVP         | Datum → CoreExpr:卫生宏(重命名式)、syntax-rules、alpha-renaming                           |
| 5   | `cambium-vm`           | lib  | MVP         | 字节码定义 + 编译器(CoreExpr→Chunk)+ 栈式 VM + 运行时值 + 原语                            |
| 6   | `cambium`              | lib  | MVP         | 门面/驱动:read→expand→compile→run 流水线、Session、prelude 装载;差异测试与 bench 挂在这里 |
| 7   | `cambium-cli`          | bin  | MVP         | clap 子命令(run/repl/compile)、rustyline REPL、诊断渲染                                   |
| 8   | `cambium-backend-wasm` | lib  | stretch(W7) | CoreExpr → wasm-GC 模块字节(wasm-encoder),含 `abi` 模块(host import 契约)                 |
| 9   | `cambium-host`         | lib  | stretch(W7) | wasmtime 嵌入:Config、运行时 imports 实现、fuel metering                                  |

目录布局:`crates/<完整包名>`(如 `crates/cambium-reader`),避免目录名与包名二义。stretch 两个 crate **W7 才加入 `members`**,现在只定契约(§3.8/§3.9),不建空壳(空壳会腐烂,Steel 半接线 JIT 的教训,architecture.md §4)。

### 1.2 依赖 DAG

```
第 0 层   cambium-common
              ↑
第 1 层   cambium-reader        cambium-core
              ↑                  ↑         ↑
第 2 层   cambium-expander ──────┘    cambium-vm    ┆cambium-backend-wasm┆
          (common+reader+core)       (common+core)  ┆   (common+core)    ┆
              ↑                          ↑          ┆        ↑           ┆
              │                          │          ┆  cambium-host      ┆
              │                          │          ┆  (backend-wasm     ┆
              │                          │          ┆   ::abi + wasmtime)┆
第 3 层   cambium(门面)──────────────────┴──[feature "wasm"]──┘
              ↑
第 4 层   cambium-cli(+ clap、rustyline、codespan-reporting/term)
```

规则:**只允许向下依赖,同层互不依赖**。`cambium-reader` 与 `cambium-core` 互不相识(Datum 与 Const 刻意分开,见 §3.3);`cambium-vm` 与 `cambium-backend-wasm` 互不相识(交接面只有 `cambium-core::{Program, Backend}`);`cambium-host` 不依赖编译器任何部分——它消费 `&[u8]`,只从 `cambium-backend-wasm::abi` 拿 import 名字/签名常量。

### 1.3 版本与发布

所有 crate `version.workspace = true`(0.0.x 同步走),共享依赖版本进 `[workspace.dependencies]`,成员用 `dep.workspace = true` 引用。lints 已由 `[workspace.lints]` 统一(unsafe deny、missing_docs warn、clippy pedantic warn),每个新 crate 的 `Cargo.toml` 必须带 `[lints] workspace = true`。

---

## 2. 边界总则(实查 Steel/scheme-rs 后提炼,证据见附录)

1. **边界用"不知道什么"定义**。§3 每个 crate 都有一张"禁止知道"清单,review 时按它拒绝依赖。反例:scheme-rs 除 proc-macro 外全部塞单 crate,`ast`/`expand`/`cps` 都是 `pub(crate)`,下游想只复用 reader 都做不到。
2. **运行时值与编译期 IR 严格分层**。`vm::Value` 不得引用 `CoreExpr`;闭包体是编译产物(`Rc<Chunk>` 片段),不是 IR 节点。反例:Steel `rvals.rs`(107KB)与求值器字段互相纠缠;scheme-rs `value.rs` 引用 `cps::` 内部类型。
3. **不用 feature flag 暂缓决策**。feature 只用于"可选子系统"(门面的 `wasm`、将来的 `bignum`),绝不用于同一职责的多套并行实现。反例:Steel 30+ features 里三套持久化集合、三套引用计数、两代 JIT 并存,测试矩阵乘性爆炸。
4. **不建与其他 crate 同名的半转发模块**。反例:`steel_core::parser` 模块与 `steel-parser` crate 同名,内容却混着一行 re-export shim 和 79KB 的宏展开器。Cambium 内已按职责命名 crate,crate 内部子模块不得再叫 `reader`/`parser` 之外加转发。
5. **枚举一处定义,派生表跟着走**。`Prim` 与 `OpCode` 各用一个声明宏在唯一权威处定义,名字/arity/栈效应表由同一宏派生,expander(内建名→Prim)与 vm(Prim→实现)消费同一张表。正面先例:Steel 把 `OpCode` 独立在近零依赖的 `steel-gen`,反汇编工具不必拉整个 VM。
6. **文件体量纪律**:单文件逼近 1000 行就必须拆模块。反例:Steel `vm.rs` 276KB、`analysis.rs` 295KB,scheme-rs `ports.rs` 141KB。机械化手段有限(clippy 只有函数级 `too_many_lines`,pedantic 已开),先靠 review 纪律,重犯再上 CI 脚本。
7. **测试语料放 `tests/`,不进 `src/`**。采用 scheme-rs 模式:`tests/*.rs` 驱动 + `tests/corpus/*.scm` 夹具;不学 Steel 把语料塞 `src/tests/` 靠目录名隐式编码预期。
8. **REPL/CLI 与核心的分离落在 crate 边界**(Steel `steel-repl` 正例、scheme-rs `main.rs` 躺主 crate 反例);将来若做 LSP,独立 `cambium-lsp` crate,依赖 reader+expander,不侵入 vm。

---

## 3. 逐 crate 设计

### 3.1 cambium-common

**职责**:整个 workspace 的最底层词汇表——源位置、符号、诊断的数据模型。

**禁止知道**:任何语法(Datum)、任何 IR、任何值表示、任何 IO。

```rust
pub struct FileId(u32);
pub struct Span { pub file: FileId, pub start: u32, pub end: u32 }
pub struct Spanned<T> { pub node: T, pub span: Span }

/// 符号 = interner 里的 u32 句柄;Copy + Eq + Hash。
pub struct Sym(string_interner::DefaultSymbol);
pub struct Interner { /* string-interner 包装 */ }
impl Interner {
    pub fn intern(&mut self, s: &str) -> Sym;
    pub fn resolve(&self, sym: Sym) -> &str;
    /// 卫生宏重命名与 IR 临时量用:生成保证不与用户符号冲突的新符号(内部计数器)。
    pub fn gensym(&mut self, prefix: &str) -> Sym;
}

/// 源文件注册表:名字 + 全文,诊断渲染(codespan-reporting 的 Files impl)与将来 DWARF 都吃它。
pub struct SourceMap { /* Vec<(name, text)>,FileId 即索引 */ }

/// 诊断数据模型直接复用 codespan-reporting 的 Diagnostic/Label(纯数据,零 IO 依赖);
/// 渲染(term::emit + termcolor)只发生在 cli。
pub type Diagnostic = codespan_reporting::diagnostic::Diagnostic<FileId>;
pub trait ToDiagnostic { fn to_diagnostic(&self) -> Diagnostic; }
```

依赖:`string-interner`、`codespan-reporting`(默认关渲染 feature)、`thiserror`。
测试:interner 往返、gensym 唯一性、Span 合并;全部纯单测。

### 3.2 cambium-reader

**职责**:源文本 → datum 层(architecture.md §5 reader 节)。手写词法器;每个节点带 Span;R7RS 大小写敏感、`#|...|#`、`#;`、`#b/#o/#x`、字符名、字符串转义;datum label 后置。

**禁止知道**:宏、求值、CoreExpr、任何后端。

```rust
pub enum Datum {
    Bool(bool), Fixnum(i64), Flonum(f64), Char(char),
    Str(String), Symbol(Sym),
    Pair(Box<Spanned<Datum>>, Box<Spanned<Datum>>), Nil,
    Vector(Vec<Spanned<Datum>>), Bytevector(Vec<u8>),
}

pub fn read_all(src: &str, file: FileId, interner: &mut Interner)
    -> Result<Vec<Spanned<Datum>>, ReadError>;

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    /// 输入在 datum 中途耗尽(括号/字符串未闭合)。REPL 据此提示续行,
    /// 是公开 API 的一部分,不是内部细节。
    #[error("unexpected end of input")]
    Incomplete { span: Span },
    // … 其余变体均带 Span,实现 ToDiagnostic
}
```

内部模块:`lexer`(token + span)、`parser`(token→datum)、`number`(数字字面量,R7RS §7.1.1 子集)。
测试:TDD 全绿是 W1 遗留验收(§6);insta 快照(`read_all` 的 Debug 树)+ 非法输入断言具体错误变体与位置。proptest 往返测试推迟到语法定型后(W6+,附录 §A.6)。

### 3.3 cambium-core

**职责**:后端无关的内核 IR 与共享 pass;`Backend` trait(可插拔边界本体)。

**禁止知道**:Datum(表面语法)、字节码、wasm、任何运行时值。

```rust
pub struct VarId(u32);            // expander alpha-renaming 保证全局唯一
pub enum BindingKind { Local, Global }   // 顶层 define 与词法绑定的槽位空间区分

/// 编译期常量(quote 的产物)。与 reader::Datum 刻意分开:Datum 带 Span、面向
/// 错误报告;Const 无 Span、面向后端。这是唯一允许的"形状重复",换来
/// reader 与 core 互不依赖(§1.2)。
pub enum Const {
    Bool(bool), Fixnum(i64), Flonum(f64), Char(char), Str(Box<str>),
    Sym(Sym), Nil, Pair(Box<Const>, Box<Const>),
    Vector(Vec<Const>), Bytevector(Vec<u8>),
}

pub enum CoreExpr {
    Const(Const), Ref(VarId), SetBang(VarId, Box<CoreExpr>),
    If(Box<CoreExpr>, Box<CoreExpr>, Box<CoreExpr>),
    Lambda { params: Vec<VarId>, rest: Option<VarId>, body: Box<CoreExpr> },
    Call { head: Box<CoreExpr>, args: Vec<CoreExpr>, tail: bool },
    LetRec { bindings: Vec<(VarId, CoreExpr)>, body: Box<CoreExpr> },
    Seq(Vec<CoreExpr>),
    PrimCall(Prim, Vec<CoreExpr>),
}

/// prim! 宏是 Prim 的唯一权威定义:一处声明 (名字, arity, 变体),
/// 同时派生 ①Prim 枚举 ②name→Prim 查找表(expander 用)③arity 表(两后端共用)。
pub enum Prim { /* cons, car, cdr, +, -, vector-ref, … 由宏生成 */ }

pub struct Program {
    pub globals: Vec<(Sym, VarId)>,   // 顶层 define 的槽位表(REPL 重定义复用槽位)
    pub body: Vec<CoreExpr>,
    pub var_meta: VarTable,           // VarId → {kind, 是否被捕获, 是否被 set!, 源名}
}

pub mod passes {
    pub fn mark_tail_calls(prog: &mut Program);       // 尾位置标注(R7RS §3.5 枚举)
    pub fn analyze_free_vars(prog: &Program) -> FreeVarMap;
    pub fn convert_assignments(prog: &mut Program);   // 被 set! 且被捕获 → 装箱
}

pub trait Backend {
    type Artifact;
    type Error: std::error::Error + ToDiagnostic;
    fn compile_program(&mut self, prog: &Program) -> Result<Self::Artifact, Self::Error>;
}
```

与 architecture.md §5 的偏离(有意):`Backend` 的错误从固定 `CompileError` 改为关联类型 `Error: ToDiagnostic`——两个后端的失败形态完全不同(VM:寄存器/深度溢出;wasm:类型段构造失败),硬共享一个枚举会变成杂物抽屉;渲染层统一靠 `ToDiagnostic`。

CPS/ANF 明确不进此 crate(architecture.md §5"刻意不做的事")。
测试:三个 pass 各自独立单测(尾位置对 R7RS §3.5 的枚举逐条断言;自由变量对手工算例;assignment conversion 前后语义等价)。

### 3.4 cambium-expander

**职责**:datum → CoreExpr,吸收一切派生形式(architecture.md §5 expander 节):内核形式 ~8 种、内建宏(let/cond/case/and/or/when/unless/do/quasiquote…)、用户 syntax-rules(重命名式卫生,`Syntax` 预留 scope-set 字段)、alpha-renaming 铸造 VarId、顶层 define 的槽位分配。

**禁止知道**:字节码、wasm、运行时值。(它知道 `Prim`——通过 core 的派生表把未被遮蔽的内建名映射到 `PrimCall`。)

```rust
/// 跨 REPL 行持久:宏表、顶层作用域(Sym→VarId)、VarId 计数器。
pub struct Expander { /* ExpandEnv */ }
impl Expander {
    pub fn new(interner: &mut Interner) -> Self;   // 装入内建宏与 prim 表
    pub fn expand_program(&mut self, data: &[Spanned<Datum>], interner: &mut Interner)
        -> Result<Program, ExpandError>;
}
```

内部模块:`env`(scoped map,只活在本 crate)、`hygiene`(rename)、`syntax_rules`(matcher/transcriber,基础省略号)、`derived`(内建宏)、`quasi`、`toplevel`(define/begin 拼接、槽位复用)。API 按"模块为单位展开"设计,给 define-library 留缝(Hoot 宏 phasing 教训,architecture.md §5)。
测试:golden 快照(datum→CoreExpr 的 pretty-print);卫生反例集(捕获、模板变量遮蔽);syntax-rules 用 Steel 通过的真实测试集子集对齐。

### 3.5 cambium-vm(MVP 的产品)

**职责**:`Backend` 的第一个实现。四块,依赖由浅入深:

```
bytecode(OpCode/Chunk/反汇编;只依赖 common+core 的 Const)
   ↑                    ↑
compile(CoreExpr→Chunk)  vm(解释循环、帧栈、TCO)
                             ↑
                    value + prims(运行时值、原语实现)
```

**禁止知道**:Datum、表面语法、wasm、wasmtime。输入只有 `core::Program`。

```rust
pub mod bytecode {
    /// opcode! 宏唯一权威定义:变体 + 操作数形状 + 栈效应注记,
    /// 派生反汇编表。MVP 纪律:≤40 个,超指令等 benchmark 说话(architecture.md §5)。
    pub enum Op { /* … */ }
    pub struct Chunk { pub code: Vec<u8>, pub consts: Vec<vm::Value>, pub spans: SpanTable }
    pub fn disassemble(chunk: &Chunk, interner: &Interner) -> String;
}

pub struct VmBackend;   // impl core::Backend<Artifact = bytecode::Chunk>

pub enum Value {
    Fixnum(i64), Flonum(f64), Bool(bool), Char(char), Nil,
    Sym(Sym),
    Str(Rc<str>),                       // 不可变字符串(决策见 §7)
    Pair(Rc<PairCell>),                 // 手写 cons cell;set-car!/set-cdr! 走 RefCell
    Vector(Rc<RefCell<Vec<Value>>>),
    Closure(Rc<Closure>),               // 持 Chunk 片段句柄 + 捕获,绝不持 CoreExpr
    // … Record、MultipleValues、原语句柄
}

pub struct Vm { /* globals 槽位表、帧栈、操作数栈;跨 chunk 持久(REPL) */ }
impl Vm {
    pub fn new() -> Self;
    pub fn run(&mut self, chunk: &Chunk, interner: &Interner) -> Result<Value, SchemeError>;
}

/// 外部表示(R7RS write 语义)。差异测试的比较点:两个后端各自产出这个字符串。
pub fn write_value(v: &Value, interner: &Interner) -> String;
```

关键实现决策沿用 architecture.md §5 表格:`loop + match u8` dispatch、自管 `Vec<Frame>`(TCO=复用帧)、第一阶段 `Rc` + 文档明示 letrec 循环泄漏、`Trace` trait 从第一天预留、NaN-boxing/混合 GC 列为 perf 里程碑。原语实现按 `prim!` 派生的 arity 表注册,错误统一走 `SchemeError`(实现 ToDiagnostic;W5 起映射 raise/guard 子集)。

测试:bytecode 反汇编 golden(insta);VM 行为测试走门面 crate 的语料(§5);`(let loop …)` 百万次迭代不涨栈是 W3 验收硬指标。

### 3.6 cambium(门面/驱动)

**职责**:唯一把全流水线接起来的地方——也是集成测试、差异测试、criterion bench 的挂载点(它们需要库级入口,这就是本 crate 的第二个具体用例,不是投机抽象)。

```rust
pub struct Session {
    interner: Interner, sources: SourceMap,
    expander: Expander, engine: EngineKind,
}
pub enum EngineKind { Vm(vm::Vm), #[cfg(feature = "wasm")] Wasm(host::Host) }
impl Session {
    pub fn new(engine: EngineKind) -> Self;          // 装载 prelude(见下)
    /// 一段源码 → 每个顶层形式的外部表示;REPL 与 runner 共用。
    pub fn eval(&mut self, src: &str, name: &str) -> Result<Vec<String>, CambiumError>;
}
```

- **prelude 归属**:标准库里用 Scheme 写的部分(`prelude.scm`:list/string 便利函数等)放本 crate 的 `scheme/` 目录,`Session::new` 时经完整流水线编译装载。vm crate 只含 Rust 原语——它没有 expander,放不了 Scheme 源(Steel 把 41KB stdlib.scm 塞进 steel-core 是因为它没有这层)。
- feature `wasm`:拉入 backend-wasm + host。MVP 构建零 wasm 依赖。
- 差异测试驻地:`tests/differential.rs` 对 `tests/corpus/**/*.scm` 逐个跑 `--backend vm|wasm` 比较 `write` 输出与 stdout(W7 起第二后端接入后生效;W5 起先做"语料 → VM 预期输出"的 golden)。

**禁止知道**:终端、clap、rustyline(那些是 cli 的)。

### 3.7 cambium-cli

**职责**:人机界面。clap 子命令 `run <file>`、`repl`、`compile <file>`(`--backend vm|wasm`,ValueEnum 校验);rustyline REPL(历史、`MatchingBracketValidator` 起步,续行判定升级为"调 reader 看是否 `Incomplete`"列为改进项);把 `ToDiagnostic` 的错误经 codespan-reporting `term::emit` 渲染到终端(带色带位置)。

依赖:`cambium`(+直接依赖 `cambium-reader`/`cambium-common` 做续行判定与渲染——Steel 的 steel-repl 同款依赖形状)、`clap`、`rustyline`、`codespan-reporting`(开 term feature)。二进制名 `cambium`。
测试:CLI 冒烟(`run` 一个夹具文件断言 stdout);REPL 逻辑薄到不值得单独测的程度,厚逻辑全部下沉门面。

### 3.8 cambium-backend-wasm(stretch,W7 进 members)

**职责**:`Backend` 第二实现,`Artifact = Vec<u8>`(wasm-GC 模块)。对象表示/调用约定/spike 验收全部按 architecture.md §9 执行,此处只定 crate 边界:

- 内部 pass:闭包转换(→ 将来 tailification 也只进这里,不污染 core)。
- `pub mod abi`:host import 契约的唯一权威——import module 名、每个 import 的名字与签名常量、`(ref eq)` 统一类型与 tag 约定的文档注释。host 只依赖这个模块。
- 发射用 `wasm-encoder`;`wasmparser`(validate)与 `wasmprinter`(golden WAT 快照)是 dev-dependencies。**不用 walrus,不碰 stringref**(architecture.md F6、§2 工具链补充)。

**禁止知道**:wasmtime(发射器不执行)、vm::Value、Datum。

### 3.9 cambium-host(stretch,W7 进 members)

**职责**:wasmtime 嵌入。pin wasmtime 46(2026-07-03 实查:最新 v46.0.1,v47 未发布),`Config` 显式 `wasm_gc(true)` + `wasm_tail_call(true)` + `wasm_exceptions(true)`(对 v46/v47 都正确,开放问题 #3 就此落定);只用 Cranelift(Winch 不支持 GC,F3)。实现 `abi` 要求的 imports(spike 最小面:字符串输出;bignum 走 num-bigint 是后续);fuel metering 作为沙箱演示接口暴露。

```rust
pub struct Host { /* Engine + Linker(imports 已接) */ }
impl Host {
    pub fn new(limits: Limits) -> Result<Self, HostError>;   // Limits { fuel: Option<u64> }
    pub fn run(&mut self, wasm: &[u8]) -> Result<String, HostError>;  // 返回程序输出
}
```

**禁止知道**:CoreExpr、编译器任何内部——输入是字节,契约是 `abi`。

---

## 4. 第三方依赖选型(2026-07-03 全部实查,详据见附录)

### 4.1 MVP 依赖

| 用途             | 选型               | 版本   | 许可证         | 落点                   | 关键理由 / 被否者                                                                                                                                                        |
| ---------------- | ------------------ | ------ | -------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Symbol interning | string-interner    | 0.20.0 | MIT/Apache-2.0 | common                 | 单线程优先、活跃;**lasso 否**:近 2 年停滞,社区公开讨论 fork                                                                                                              |
| 诊断数据+渲染    | codespan-reporting | 0.13.1 | Apache-2.0     | common(数据)/cli(渲染) | 数据与渲染分层正合 §1.2;曾 4 年空窗、2025 起复活;**ariadne 否**:GitHub 已 archived 迁 Codeberg,crates.io 9 个月未发版;**miette 否**:渲染标注焊进错误类型,默认 feature 重 |
| 错误类型 derive  | thiserror          | 2.0.18 | MIT/Apache-2.0 | 各 crate               | 行业默认;每 crate 自有错误枚举 + ToDiagnostic                                                                                                                            |
| REPL 行编辑      | rustyline          | 18.0.1 | MIT            | cli                    | 内置 MatchingBracketValidator 直接解决括号续行;**reedline 否**:必需依赖含 serde/crossterm,为用不上的高亮付重量                                                           |
| CLI 解析         | clap(derive)       | 4.6.1  | MIT/Apache-2.0 | cli                    | 子命令+ValueEnum 正是需求形状;**pico-args 否**:4 年冻结                                                                                                                  |
| 快照测试         | insta              | 1.48.0 | Apache-2.0     | dev-deps               | 事实标准;cargo-insta 1.48.0 已在 mise.toml 钉版,版本对齐                                                                                                                 |
| 微基准           | criterion          | 0.8.2  | Apache/MIT     | 门面 dev-deps(W6)      | **divan 否**:14 个月未发版,维护质疑 issue 无维护者回应;端到端用已钉的 hyperfine                                                                                          |

### 4.2 明确不引入(手写)

| 场景                 | 决定                              | 理由                                                                                                                                  |
| -------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| cons cell / 持久列表 | 手写 `Rc<PairCell>`(RefCell 车厢) | im 已 archived 且 MPL;rpds/im-lists 是纯持久结构,与 `set-car!`/环状列表的可变语义相反,引入后仍要套 Rc\<RefCell\> 等于白拿一套结构共享 |
| VarId 等索引         | 手写 newtype + `Vec`              | 只有两三种索引;typed-index-collections 留作类型超过 ~5 种时的备选                                                                     |
| property testing     | 暂不引入                          | reader 语法定型(W6)后上 proptest 1.11;cargo-fuzz 需 nightly,与 stable 锁定冲突,真要用时放独立 `fuzz/` workspace                       |

### 4.3 stretch(W7 才进 Cargo.toml)与 future

| 用途                            | 选型                      | 版本(实查)     | 备注                                                                                                   |
| ------------------------------- | ------------------------- | -------------- | ------------------------------------------------------------------------------------------------------ |
| wasm 发射                       | wasm-encoder              | 0.252.0        | Apache-2.0 WITH LLVM-exception(白名单已含)                                                             |
| wasm 验证/打印                  | wasmparser / wasmprinter  | 0.252.0        | dev-deps                                                                                               |
| 运行时                          | wasmtime                  | pin 46(46.0.1) | MSRV 1.94 ✓;v47 未发布,不等(Config 显式设置两版通吃)                                                   |
| bignum/rational(future feature) | num-bigint + num-rational | 0.5.0 / 0.4.2  | **malachite 否(LGPL-3.0-only)、rug 否(LGPL + GMP C 依赖)**——deny.toml 白名单直接拒绝;Steel 同用 num 系 |

---

## 5. 测试与语料组织

1. **单元测试**:各 crate `#[cfg(test)]` 内联,按用户测试三件套(正例断言具体值/负例断言具体错误变体/边界)。
2. **快照(insta)**:reader 的 datum 树、expander 的 CoreExpr pretty-print、vm 的反汇编、(W7)backend-wasm 的 golden WAT。评审走 `cargo insta review`(工具已钉版)。
3. **语料库**:`crates/cambium/tests/corpus/**/*.scm`,一个 `.scm` 一个行为;预期输出写在同名 `.expected` 或文件头注释指令(`;; => `)里,由 `tests/corpus.rs` 驱动——**不是**靠目录名隐式编码预期(§2.7)。来源:自写 + Chibi 行为对照 + ecraven/r7rs-benchmarks 子集(architecture.md §5)。
4. **差异测试**(W7 起):同一语料喂两个 `Backend`,比较 `write_value` 输出与 stdout。这是双后端架构的最大红利,驱动器就在门面 crate。
5. **基准**:criterion(进程内微基准:dispatch、值操作)在门面 `benches/`;hyperfine(端到端,对 Steel/Chibi 的横向对比)脚本进 `bench/` 目录 + just recipe(W6)。

---

## 6. 工作拆分(对 architecture.md §7 周计划的 crate 级落地)

> 基线修正:W1 的 reader 部分未开工(workspace 无任何 crate),下表把它并入本周补齐。

| 周                | crate               | 任务(验收标准)                                                                                                                                                                                                          |
| ----------------- | ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **W1 遗留(立即)** | common              | crate 建立;Interner(intern/resolve/gensym)、Span/Spanned/FileId、SourceMap、ToDiagnostic;单测全绿                                                                                                                       |
|                   | reader              | lexer+parser+number 三模块;`read_all` 过 TDD 用例集(`(a . b)` 结构断言、非法输入带位置错误、`Incomplete` 变体);insta 快照就位                                                                                           |
| **W2**            | core                | CoreExpr/Const/Prim(prim! 宏)/Program/VarTable;三 pass 各自独立测试;`Backend` trait 定稿                                                                                                                                |
|                   | expander            | 内核 8 形式 + alpha-renaming + 顶层槽位;let/let\*/cond/and/or/when/unless 内建宏;卫生反例集起步                                                                                                                         |
| **W3 第一验收:垂直切片** | 全链路 | 最小词法子集 + 8 内核形式 + 最小 opcode 集跑通 `((lambda (x) (+ x 1)) 41)` 量级程序,验证 Datum/CoreExpr/Backend 三条接缝;语料 runner(`;; =>` 驱动,§5.3)骨架随切片同步启用——语言级 TDD 自此可用,后续特性一律先写语料再实现(2026-07-06 决策,提前于 W5 的全面接入) |
| **W3**            | vm                  | bytecode 模块(opcode! 宏,≤40)+ compile + 解释循环;闭包/TCO/fixnum+flonum 算术;百万次尾递归不涨栈;反汇编 golden                                                                                                          |
|                   | cambium + cli       | Session::eval 最小面 + REPL 跑通(rustyline + 括号续行)——"W3 REPL 跑通"的验收在 cli,不在 vm                                                                                                                              |
| **W4**            | expander            | syntax-rules(基础省略号)+ quasiquote                                                                                                                                                                                    |
|                   | vm/core             | define-record-type(prim + RecordType 值)、多返回值                                                                                                                                                                      |
| **W5**            | vm                  | raise/guard 子集(SchemeError→Scheme 异常对象);标准库:Rust 原语补齐 + 门面 `scheme/prelude.scm`                                                                                                                          |
|                   | cambium             | 语料库接入(corpus 驱动 + golden);诊断渲染打磨                                                                                                                                                                           |
| **W6**            | cambium             | criterion 微基准 + hyperfine 端到端(ecraven 子集);README/文档;proptest 进 reader(语法已定型)                                                                                                                            |
| **W7–8(弹性)**    | backend-wasm + host | 首选 AOT spike,验收=architecture.md §9(fib/百万迭代不涨栈/闭包/字符串常量,过 `wasm-tools validate --features gc`,wasmtime 跑通,与 VM 语料输出一致);此时 wasm 系依赖进 workspace、mise.toml 取消注释 wasmtime/wasm-tools |

每周的"加特性改动半径"预期见 architecture.md §6——若某特性实现时改动半径超出该表(例如加 record 动到了 CoreExpr),按红旗停下来重审边界。

---

## 7. 本设计落定/新立的决策(ADR 候选)

| 决策                                              | 状态                                  | 备注                                                                                                                                         |
| ------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| wasmtime pin v46 + Config 显式开三提案            | **落定**(architecture.md 开放问题 #3) | v47 未发布是实查事实;显式 Config 两版通吃                                                                                                    |
| 字符串不可变(`Str(Rc<str>)`,不支持 `string-set!`) | **本文档新立,建议 ADR**               | 偏离 R7RS 需在 README 明示;Steel 同款;AOT `(array i8)` 天然亲和(开放问题 #4)                                                                 |
| Backend::Error 为关联类型而非共享 CompileError    | 本文档新立(§3.3)                      | 与 architecture.md §5 草图的有意偏离                                                                                                         |
| Prim/OpCode 宏定义 + 派生表                       | 本文档新立(§2.5)                      | steel-gen 正例                                                                                                                               |
| 9-crate 边界与依赖 DAG                            | 本文档主体                            | 若要正式 ADR,引用本文即可,不要复述                                                                                                           |
| 重写 `docs/adr/0001`、`0002`                      | **已完成(2026-07-06)**               | 初版 ADR 曾与初版 crates 一起有意删除;已按新设计重写落地于 `docs/adr/`,justfile/mise.toml 的引用不再悬空 |

---

## 附录 A:调研来源与方法

- **A.1 竞品组织**:GitHub API 拉取 Steel(0.8.2,21-crate workspace)与 scheme-rs(0.2.0,单 crate + proc-macro)完整文件树与关键 Cargo.toml/源文件,2026-07-03。§2 各反例的文件名与字节数均出自该次实查。
- **A.2 wasm 工具链**:GitHub releases + docs.rs 实查(wasmtime 46.0.1/2026-06-24 为最新,v47 未发布;wasm-tools 家族 0.252.0;许可证 Apache-2.0 WITH LLVM-exception;wasmtime MSRV 1.94)。
- **A.3 依赖选型**:crates.io API + 各仓库 issue 实查(lasso 停滞证据:维护状态讨论 issue 无维护者回应;divan 同类 issue #91;ariadne archived;malachite/rug 许可证)。
- **A.4 上游阅读材料总表**:architecture.md §10。按 crate 的切片:reader → R7RS §7 词法 + Crafting Interpreters "Scanning";expander → Clinger/Rees《Macros That Work》+ Steel `rename_idents.rs`(+ 远期 Flatt sets-of-scopes);core → Dybvig《Three Implementation Models》+ Ghuloum 增量编译器 + Clinger 尾调用论文;vm → Crafting Interpreters clox 各章 + Steel `vm.rs`(帧/尾调用/ContinuationMark)+ `passes/analysis.rs`(move 优化,perf 里程碑);backend-wasm → wingolog "a world to win" + Hoot tailify.scm + GC MVP.md + Binaryen GC Guidebook;host → wasmtime embedding/fuel/DWARF 文档 + Wastrel import 清单(wingolog 2026-03-31)。
- **A.5 本地技能**:`compiler-frontend`(词法/AST/符号表)、`interpreters`(dispatch/值表示/栈管理)、`wasm-wasmtime`(embedding/fuel/调试)已在设计中消化;`jit-compilation`、`abi-and-calling-conventions` 在 JIT/AOT 深化时再取用。
- **A.6 推迟项备忘**:proptest 1.11.0(W6 后)、cargo-fuzz(需 nightly,独立 fuzz workspace)、typed-index-collections 3.5.0(索引类型 >5 种时)、rpds 1.2.1(若将来要 Scheme 层持久 vector)。
