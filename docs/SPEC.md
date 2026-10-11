# Helix Engine Specification (condensed)

| | |
|---|---|
| Version | 0.9 (bootstrap edition) |
| Reference implementation | this repository |
| Keywords | MUST / MUST NOT / SHOULD / MAY per RFC 2119 |
| Stability | Requirement IDs are immutable once published (VER-1) |

## 0. How to read

Status: **impl** (working in this repo) / **roadmap** (specified, not built).
Verification tags: **[T]** unit test · **[H]** headless digest · **[G]**
golden image (roadmap) · **[B]** benchmark (roadmap) · **[I]** inspection.

## 1. What Helix is

A data-oriented 3D engine in Rust over wgpu (Vulkan / DX12 / Metal / GL /
WebGPU). Hybrid: ECS gameplay state + stateful services, joined by four
bridges.

```text
        ECS WORLD                        SERVICES
     components (plain data)       Renderer (thread-bound), Input,
     systems (ordered queries)     Cvars, AssetServer
          |  (1) HANDLES  MeshId / Handle<T>    ^
          |  (2) EXTRACTION  FrameData          |
          v                                     |
     (3) EVENTS  typed cursor bus --------------+
     (4) CTX  &mut World + sim services -> systems
```

Goals: determinism, cache-friendly ECS, explicit graphics, headless-first,
sanctioned escape hatches, safe.

## 2. Architecture

```text
                 +--------------+
                 |  helix-core  |  math, hash, config, cvars
                 +------+-------+
                        v
                 +--------------+        +---------------+
                 |  helix-ecs   |        | helix-assets  |
                 +---+------+---+        +-------+-------+
             +-------+      +----------------+  |
             v       v      v                v  v
        +-------+ +--------+ +----------+ +-----------+
        | scene | |  sim   | |  render  | |   app     |  (+ bin)
        +---+---+ +---+----+ +----+-----+ +-----+-----+
            +---------+---------^----+--------+
        sim -> scene ONLY (one-way); render consumes extraction types
```

| ID | Rule | St | Tag |
|---|---|---|---|
| ARCH-1 | ECS depends on no service type; Ctx holds only sim services | impl | [I] |
| ARCH-2 | Renderer consumes only extraction types | impl | [I] |
| ARCH-3 | Crate DAG above; sim -> scene one-way | impl | [I] |
| ARCH-4 | GPU/OS handles never in components (MeshId only) | impl | [I] |
| ARCH-6 | sim tree has no wgpu/winit — CI `cargo tree` gate | impl | [T] |
| THR-1 | Renderer owned by render thread; moved by value after main-thread build | impl | [I] |
| THR-2 | Cross-thread data is plain `Send`; no World references | impl | [I] |
| MOD-1..3 | Modules are the ONLY extension path; topo build; dup/cycle/unknown-dep = boot error | impl | [T] |
| BOOT-1 | Ordered boot phases, one log line each | impl | [I] |
| FRM-1/2 | Before/after-stage hooks outside the system list | impl | [I] |

## 3. Time & the frame loop (TIME)

```text
 acc += clamp(frame_dt, 0, 0.25)
 while acc >= FIXED_DT (1/60), steps < MAX_SIM_STEPS (5):
     Input -> Sim            (dt = 1/60 exactly)
 overflow: acc %= FIXED_DT + warn          (spiral guard)
 alpha = acc / FIXED_DT
 Late (hierarchy) -> camera ease (interp) -> extract -> channel(2) -> render
```

TIME-1 sim dt = 1/60 exactly (impl [T]) · TIME-2 Late once/frame with
measured dt (impl) · TIME-3 dt clamp (impl) · TIME-4 spiral guard (impl) ·
TIME-6 zero-step frames still interpolate (impl) · TIME-7 dev title overlay
(impl).

## 4. ECS

| ID | Rule | St | Tag |
|---|---|---|---|
| ECS-1/2 | Packed entity; generation bump on despawn invalidates handles | impl | [T] |
| ECS-3 | Components 'static plain data, type-erased per TypeId | impl | [I] |
| ECS-4 | Sparse sets: dense + sparse map, swap-remove | impl | [T] |
| ECS-5 | Insert on dead entity panics | impl | [T] |
| ECS-6 | n-way joins over the smallest store | impl | [I] |
| ECS-7 | No structural change mid-iteration (snapshot-mutate; cmd buffers P2) | impl | [I] |
| ECS-8 | Iteration order unspecified | impl | [H] |
| ECS-9/10 | Stages Input->Sim->Late; Late once/frame; registration order | impl | [T] |
| ECS-12/13 | state_hash: XOR of per-entity FNV over registry-HASHED comps | impl | [H] |
| CHG-1 | Tick stamps + changed() (best-effort) | impl | [T] |

## 5. Transforms & interpolation (HIER/INTP)

HIER-1 local Transform; GlobalTransform computed · HIER-3 depth-sorted
stable propagation · HIER-4 global = parent * local (scale composes!) ·
HIER-5 dangling parent => child renders as root · HIER-6 loader-rejected
cycles — all impl [T].
INTP-1a capture_prev first in Sim · INTP-2 lerp/slerp/scale (never naive
Mat4 lerp) · INTP-3a snap on teleport (5 m) · INTP-4 extraction pure ·
INTP-5a camera eases toward interpolated pose — all impl [T].

## 6. Determinism & randomness (RNG/DETERM)

RNG-1 one World-owned stream (the `__rng` entity component) · RNG-2
xoshiro256++, stream semantics frozen · RNG-3 branch(entity), side-effect-
free — impl [T]. DETERM: identical seed + timeline => identical digest.
Digest fixtures under `tests/digests/` are load-bearing (first run records,
then verify; drift fails until deliberately regenerated).

## 7. Events (EVT)

EVT-1 cursor readers, exactly-once, two-update lifetime, late readers see
only the future (impl [T]) · EVT-5 CollisionEvent (impl subset) · EVT-7
granularity documented (windowed = frame, headless = step) · EVT-6 bus is
never a renderer feed — extraction only.

## 8. Input (INP/ACT)

INP-1 snapshot · INP-2 focus-loss clears · ACT-1..4 axis-pair bindings from
RON with embedded fallback; unknown keys warn + skip; Sim reads ActionState
only (impl [T]) — which is what makes timelines able to drive the sim.

## 9. Scenes (SCN)

SCN-1 RON docs, engine-agnostic, unit markers · SCN-2 identity defaults
(scale 1, never 0) · SCN-3 name-based parents, forward refs · SCN-4 hard
error taxonomy (parse / mesh / dup-name / unknown-parent / self / cycle) ·
SCN-5 embedded fallback + file override — impl [T]. SCN-9 version field: P2.

## 10. Rendering (REN/PBR/RES/ZB/FG)

| ID | Rule | St | Tag |
|---|---|---|---|
| REN-1 | One device/queue/surface; vsync default (config) | impl | [I] |
| REN-2 | Prefer sRGB swapchain format | impl | [I] |
| REN-3 | Depth + culling state per DEPTH consts | impl | [I] |
| REN-6 | sync_channel(2); blocking send = backpressure | impl | [I] |
| REN-7 | Protocol state machine incl. resize-race heal | impl | [I] |
| REN-8 | Dead render thread never hangs the loop | impl | [T] |
| REN-9 | FrameData plain data = extraction boundary | impl | [I] |
| REN-13 | Frustum culling (Gribb-Hartmann, depth-aware) | impl | [T] |
| REN-14 | Stable front-to-back opaque sort | impl | [T] |
| REN-16 | Camera position as uniform (never hardcoded) | impl | [I] |
| PBR-3/6 | HDR intermediate -> ACES -> single sRGB encode | impl | [I] |
| RES-1 | render_scale sizes HDR target; UI native | impl | [I] |
| ZB-1/2 | Reverse-Z + Depth32Float default; forward-Z fallback | impl | [T] |
| ZB-3 | Depth direction invisible outside render | impl | [T] |
| FG-1..3 | Declared graph, boot validation, import/transient rules | impl | [T] |

Roadmap: CSM shadows (SHD), light extraction + clustered forward (LIT),
UiPass (UI), glyph atlas, GPU particle instancing, golden images, ring
buffer uniforms, pipeline cache, parallel scheduler.

## 11. Persistence (SAVE)

SAVE-1 whitelisted component snapshots via the registry SAVE flag;
computed state never serialized · SAVE-2 HELIXSV container
(magic/version/len/hash) · SAVE-3 slot-map parent remap · SAVE-4 version
check + migrations table · SAVE-5 boundary-only apply · SAVE-6 atomic write
(tmp + fsync + rename) — impl [T]. Seed header reserved (RNG-4).

## 12. Collision (COL — mini physics)

COL-2 post-integration depenetration + slide on xz circles, self excluded,
deterministic event order · COL-3 degenerate push +x — impl [T].
helix-physics supersedes at P2.

## 13. FX

FX-1 particle entropy ONLY from WorldRng => digest-covered,
replay-identical (impl [T]/[H]) · FX-2 fixed cap, steal-oldest, alloc-free
update (impl) · FX-3 GPU instancing: roadmap (pools simulated + digested).

## 14. Testing (TST)

TST-11 tiers: T1 inline unit · T2 integration (tests/) · T3 hermetic [H]
scenario+digest (data-driven from tests/manifest.ron) · T4 budgets
(roadmap) — T1–T3 impl · TST-12 manifest = expectation registry with
intrinsic gates (all / linux_only / gpu) · TST-14 one fixture lifecycle
(record on missing; drift names the regeneration command) — impl.
TST-8/9/15/16: roadmap.

## 15. Errors (ERR)

Panics for broken engine invariants (dead-entity insert, unknown mesh at
draw); Result + log + degrade for environmental/authoring errors (level
parse, missing file, surface Lost/Outdated, corrupt saves) — impl.

## 16. Roadmap

P2: CSM shadows, lights extraction + clustered forward, UiPass, command
buffers, glyph atlas UI, asset server, budget benches, golden images.
P3: behaviors, physics crate, audio, editor protocol, WASM.

## 17. ADR index (condensed)

001 hybrid ECS + services · 002 fixed step + drop-backlog · 003 bounded
channel render feed · 006 plain-data extraction · 010 World-owned RNG ·
015 frame-level PrevGlobal interpolation · 017 thin bin · 018 XOR digest
with explicit registration · 019 sRGB swapchain preferred · 021 schema ids
not TypeId · 023 modules as sole extension path · 036 reverse-Z default ·
041 render never feeds Sim.
