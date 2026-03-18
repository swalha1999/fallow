# Fallow Codebase Health Roadmap

> Last updated: 2026-03-18

Fallow's core pipeline — parse (Oxc) → resolve → module graph — is the expensive part of static analysis. Every new analysis feature is a cheap query on that already-computed graph and AST. This roadmap extends Fallow from "dead code + duplication finder" into a **comprehensive codebase health toolkit** — all from a single binary, single config file, single CI step.

This roadmap is separate from the [core roadmap](./ROADMAP.md) which covers dead code analysis, duplication detection, integrations, and editor experience. The features here are additive and can be developed in parallel.

---

## Architecture: Single Binary, Multiple Subcommands

All health features ship as subcommands of the `fallow` binary. They share the core pipeline and add analysis passes on top.

```
fallow check          # dead code analysis (existing)
fallow dupes          # duplication detection (existing)
fallow deps           # circular deps + architecture boundaries (new)
fallow health         # complexity metrics + test gap analysis (new)
fallow viz            # dependency graph visualization (new)
fallow diff           # API surface changes between git refs (new)
```

**Why not separate tools:**

- All features share the same expensive pipeline (parse → resolve → graph). Splitting means duplicating work or building a shared library that's just fallow-core
- Users want one install, one config file, one CI step
- The module graph is the moat — every new feature is a cheap query on expensive-to-build data

**Code organization:**

New analysis modules live in `crates/core/src/` alongside `analyze.rs`:

```
crates/core/src/
  cycles.rs        — Circular dependency detection (Tarjan's SCC)
  complexity.rs    — Cyclomatic + cognitive complexity AST visitor
  boundaries.rs    — Architecture boundary rule engine
  test_gaps.rs     — Static test coverage gap analysis
  treeshake.rs     — Treeshakeability lint
  api_surface.rs   — Export snapshot + diff
```

CLI subcommands in `crates/cli/src/` follow the existing pattern.

---

## Competitive Landscape

| Category | Best Existing Tool | Language | Weakness | Fallow Advantage |
|----------|--------------------|----------|----------|-----------------|
| Circular deps | dependency-cruiser | JS | Slow on large codebases, complex config | Graph already built, zero extra parsing |
| Complexity | SonarQube / FTA | Java / Rust+SWC | Enterprise pricing / wrong parser | Oxc AST already parsed, free |
| Test gaps | Sealights | SaaS | Requires runtime instrumentation, $$$ | Static analysis from existing graph |
| Architecture boundaries | eslint-plugin-boundaries | JS | ESLint speed, limited to lint rules | Graph-level validation, fast |
| Visualization | Nx Graph | JS | Locked to Nx ecosystem | Standalone, any JS/TS project |
| API surface diff | API Extractor | JS | Complex, Microsoft-centric | Lightweight CLI, reuses export data |
| Treeshakeability | (nothing) | — | No tool does pre-build analysis | Novel capability |
| Monorepo health | Sherif | Rust | Version consistency only | Comprehensive: versions + deps + boundaries |

---

## Phase 1: Graph Queries

Low effort, high impact. Pure graph operations on existing data — zero new parsing.

### `fallow deps` — Dependency Analysis

#### Circular Dependency Detection

Detect import cycles using Tarjan's strongly connected components algorithm on the existing `ModuleGraph` edges.

**Output:**
```
Circular dependencies found (3 cycles):

  src/auth/login.ts → src/auth/session.ts → src/auth/login.ts
  (2 modules, shortest path)

  src/api/users.ts → src/api/permissions.ts → src/api/roles.ts → src/api/users.ts
  (3 modules)

  src/store/actions.ts → src/store/reducers.ts → src/store/selectors.ts → src/store/actions.ts
  (3 modules)
```

**Flags:**
- `--max-depth N` — only report cycles involving more than N modules
- `--fail-on-cycles` — exit code 1 when cycles found (CI gate)
- `--format json|human|compact|sarif` — reuse existing report infrastructure

**Config:**
```toml
[deps]
max_cycle_depth = 10
ignore_cycles = ["src/generated/**"]
```

### `fallow viz` — Visualization Output

Serialize the module graph for external visualization tools.

**Formats:**
- **DOT** — Graphviz-compatible, pipe to `dot -Tsvg` for SVG output
- **Mermaid** — Paste into GitHub markdown, Notion, or any Mermaid renderer
- **JSON** — Nodes + edges for D3.js, Cytoscape.js, or custom tooling

**Flags:**
- `--focus <path>` — subgraph rooted at a specific module or directory
- `--depth N` — limit traversal depth from focus point
- `--cluster-by directory|package` — group nodes by folder or workspace package
- `--include-external` — show npm package nodes (collapsed by default)

**Example:**
```bash
fallow viz --format mermaid --focus src/api/ --depth 2 > api-deps.md
fallow viz --format dot | dot -Tsvg > full-graph.svg
fallow viz --format json --cluster-by package > graph.json
```

---

## Phase 2: AST Analysis

New AST visitors that piggyback on the existing parallel parse phase.

### Complexity Metrics (add to `fallow health`)

#### Cyclomatic Complexity

Count control flow branch points per function: `if`, `else if`, `switch case`, `for`, `while`, `do`, `&&`, `||`, `??`, `?.`, ternary `?:`, `catch`. Score = branches + 1.

#### Cognitive Complexity

Weighted scoring per the SonarSource algorithm:
- +1 for each `if`, `else if`, `else`, `switch`, `for`, `while`, `do`, `catch`, ternary, logical operator chain
- +1 nesting increment for each level of nesting
- No increment for `else` at the same level

#### Output

```
Complexity report (highest first):

  src/api/permissions.ts:calculateAccess    cyclomatic: 23  cognitive: 31  ⚠ high
  src/utils/parser.ts:parseExpression       cyclomatic: 18  cognitive: 27  ⚠ high
  src/auth/validate.ts:validateToken        cyclomatic: 12  cognitive: 15

  Project summary:
    Files analyzed: 847
    Functions analyzed: 3,291
    Avg cyclomatic: 4.2    Avg cognitive: 5.8
    P90 cyclomatic: 12     P90 cognitive: 18
    Above threshold: 14 functions
```

**Flags:**
- `--max-cyclomatic N` — threshold for flagging (default: 15)
- `--max-cognitive N` — threshold for flagging (default: 25)
- `--sort cyclomatic|cognitive|lines` — ranking order
- `--top N` — show only the N worst offenders
- `--format json|human|compact` — reuse existing infrastructure

**Config:**
```toml
[health]
max_cyclomatic = 15
max_cognitive = 25
ignore_complexity = ["src/generated/**", "**/*.test.ts"]
```

### Architecture Boundary Enforcement (add to `fallow deps`)

Define import rules between directory-based layers. Validated against the module graph — no per-file linting overhead.

**Config:**
```toml
[[boundary]]
from = "src/ui/**"
deny = ["src/db/**", "src/server/**"]
message = "UI layer cannot import database or server code"

[[boundary]]
from = "src/features/*/**"
deny = ["src/features/*/**"]
message = "Features must not import from other features — use shared/"

[[boundary]]
from = "src/shared/**"
deny = ["src/features/**"]
message = "Shared code cannot depend on feature code"
```

**Output:**
```
Architecture boundary violations (5):

  src/ui/UserCard.tsx → src/db/queries.ts
    Violates: "UI layer cannot import database or server code"

  src/features/billing/invoice.ts → src/features/auth/session.ts
    Violates: "Features must not import from other features — use shared/"
```

**Flags:**
- `--fail-on-violations` — exit code 1 (CI gate)
- Respects `--changed-since` for incremental checking

---

## Phase 3: Novel Capabilities

Features that don't exist in the open-source JS/TS ecosystem.

### Static Test Coverage Gap Analysis (add to `fallow health`)

Identify exports and modules with no test file dependency — without running tests.

**How it works:**
1. Classify test files using existing plugin knowledge (Vitest, Jest, Playwright, Cypress, Mocha, Ava patterns) + filename heuristics (`*.test.*`, `*.spec.*`, `__tests__/`)
2. From the module graph, compute which source exports are reachable from at least one test file
3. Report exports and files with zero test reachability

**Output:**
```
Test coverage gaps (static analysis):

  src/billing/calculate.ts
    ✗ calculateDiscount (no test imports this export)
    ✗ applyTax (no test imports this export)
    ✓ calculateTotal (imported by src/billing/__tests__/calculate.test.ts)

  src/utils/crypto.ts
    ✗ entire file has no test dependency

  Test coverage score: 73% of exports reachable from test files
  Files with no test dependency: 24 / 847
```

**Flags:**
- `--changed-since <ref>` — only report gaps in changed files (the CI use case: "your PR adds untested exports")
- `--min-coverage N` — fail if score drops below N% (CI gate)
- `--ignore-test-gaps <glob>` — exclude files from gap analysis

**Config:**
```toml
[health.test_gaps]
min_coverage = 80
ignore = ["src/generated/**", "src/types/**"]
```

### Treeshakeability Lint (add to `fallow health`)

Statically detect patterns that prevent bundler tree-shaking.

**Detections:**
- **Module-scope side effects** — top-level statements that aren't declarations or exports (e.g., `console.log()`, `polyfill()`, assignments to globals)
- **CJS in ESM context** — `module.exports` / `require()` in packages with `"type": "module"`
- **Barrel file impact** — "This barrel re-exports 47 modules — importing one symbol pulls the entire chain" (quantified using existing re-export chain data)
- **Missing `sideEffects` field** — packages without `"sideEffects": false` that could benefit
- **Default export of large object** — `export default { ... }` with many properties prevents per-property tree-shaking

**Output:**
```
Treeshakeability issues (4):

  src/index.ts (barrel)
    Re-exports 47 modules — consumers importing a single symbol may pull all 47
    Suggestion: use direct imports or mark with sideEffects: false

  src/utils/polyfills.ts
    Module-scope side effect on line 3: initPolyfills()
    This prevents tree-shaking of the entire module

  package.json
    Missing "sideEffects" field — bundlers will assume all files have side effects
```

---

## Phase 4: Library & Monorepo Tools

Higher effort, targeted at library authors and monorepo teams.

### `fallow diff` — API Surface Changes

Compare exported API signatures between git refs to detect breaking changes.

**How it works:**
1. Snapshot all public exports: function name, parameter count, type annotations, class members, enum variants
2. Diff snapshots between two git refs
3. Classify changes by semver impact

**Output:**
```bash
$ fallow diff main..HEAD

API changes (main → HEAD):

  BREAKING (major):
    src/api/client.ts
      ✗ removed export: createLegacyClient
      ✗ changed: fetchUser (3 params → 2 params)

  ADDITIONS (minor):
    src/api/client.ts
      + new export: createClient
      + new export: ClientOptions (type)

  PATCHES (patch):
    src/utils/format.ts
      ~ added optional param: formatDate(date, options?)

  Suggested version bump: major
```

**Flags:**
- `--from <ref>` / `--to <ref>` — git refs to compare (default: `main..HEAD`)
- `--scope <glob>` — limit to specific paths
- `--format json|human` — structured output for CI

**Limitations (documented):**
- Syntactic analysis only — cannot detect type narrowing/widening without a type checker
- Catches ~70% of breaking changes: removed/renamed exports, changed arities, removed members
- Does not detect changes in return types or generic parameter changes

### Monorepo Health (add to `fallow health`)

Comprehensive workspace health checks beyond what Sherif covers.

**Checks:**
- **Version drift** — workspace packages depending on different versions of the same dependency
- **Circular workspace deps** — cycle detection at the workspace package level
- **Unused workspace deps** — workspace package listed in dependencies but never imported cross-workspace
- **Internal version mismatches** — workspace packages referencing each other with outdated version ranges

**Config:**
```toml
[health.monorepo]
enforce_consistent_versions = true
allow_version_drift = ["typescript", "@types/*"]  # some deps are OK to vary
```

**Flags:**
- `--workspace <name>` — scope to one workspace package

---

## Scoring Matrix

Each feature scored on **Value** (developer demand × market gap) and **Feasibility** (existing infrastructure leverage × implementation effort). Score = Value × Feasibility, normalized to 100.

| # | Feature | Value | Feasibility | Score | Effort | Phase |
|---|---------|-------|-------------|-------|--------|-------|
| 1 | Circular dependency detection | 9 | 10 | **95** | S | 1 |
| 2 | Complexity metrics | 9 | 9 | **90** | M | 2 |
| 3 | Static test coverage gaps | 10 | 8 | **88** | M | 3 |
| 4 | Architecture boundaries | 8 | 9 | **85** | M | 2 |
| 5 | Monorepo health | 7 | 9 | **80** | M | 4 |
| 6 | Visualization output | 7 | 9 | **80** | M | 1 |
| 7 | API surface diff | 8 | 6 | **72** | L | 4 |
| 8 | Treeshakeability lint | 7 | 7 | **70** | M | 3 |

Effort: **S** = days, **M** = weeks, **L** = months.

---

## What This Enables

After Phase 3, Fallow becomes: **"The fast codebase health toolkit for JS/TS"**

One binary answers:
- What code is dead? → `fallow check`
- What code is duplicated? → `fallow dupes`
- What imports are circular? → `fallow deps`
- What code is too complex? → `fallow health`
- What code is untested? → `fallow health --test-gaps`
- Does my architecture hold? → `fallow deps --boundaries`
- What does my codebase look like? → `fallow viz`
- Did I break my API? → `fallow diff`

All running in seconds on projects where competing tools take minutes, all from a single `fallow.toml` config, all with JSON/SARIF output for CI.

No single competitor covers this combination at Rust-native speed.
