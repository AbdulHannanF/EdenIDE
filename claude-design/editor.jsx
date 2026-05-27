/* EDITOR.JSX — the hero. Main code editor surface. */

const { useState, useEffect, useRef, useMemo, useCallback } = React;

/* ───────────── syntax highlighting (rust) ───────────── */
const RUST_KW = new Set([
  "as","async","await","break","const","continue","crate","dyn","else","enum","extern",
  "false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref",
  "return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while"
]);
const RUST_TYPE = /^(?:[A-Z][A-Za-z0-9_]*|u8|u16|u32|u64|usize|i8|i16|i32|i64|isize|f32|f64|bool|str|String|Vec|Box|Arc|Rc|Result|Option|Duration|Instant)$/;

function tokenizeRust(line) {
  const out = [];
  let i = 0;
  while (i < line.length) {
    const c = line[i];
    // line comment
    if (c === "/" && line[i+1] === "/") {
      out.push({ t: "cm", v: line.slice(i) });
      break;
    }
    // doc comment
    if (c === "/" && line[i+1] === "*") {
      const end = line.indexOf("*/", i+2);
      const stop = end === -1 ? line.length : end+2;
      out.push({ t: "cm", v: line.slice(i, stop) });
      i = stop; continue;
    }
    // string
    if (c === '"') {
      let j = i+1;
      while (j < line.length && line[j] !== '"') { if (line[j] === "\\") j++; j++; }
      out.push({ t: "st", v: line.slice(i, j+1) });
      i = j+1; continue;
    }
    // char
    if (c === "'" && /[a-zA-Z_]/.test(line[i+1]) && line[i+2] !== "'") {
      // lifetime
      let j = i+1; while (j < line.length && /[a-zA-Z0-9_]/.test(line[j])) j++;
      out.push({ t: "lf", v: line.slice(i, j) });
      i = j; continue;
    }
    // number
    if (/[0-9]/.test(c)) {
      let j = i; while (j < line.length && /[0-9_.]/.test(line[j])) j++;
      // suffix
      while (j < line.length && /[a-zA-Z0-9]/.test(line[j])) j++;
      out.push({ t: "nm", v: line.slice(i, j) });
      i = j; continue;
    }
    // ident
    if (/[a-zA-Z_]/.test(c)) {
      let j = i; while (j < line.length && /[a-zA-Z0-9_]/.test(line[j])) j++;
      const word = line.slice(i, j);
      let kind = "id";
      if (RUST_KW.has(word)) kind = "kw";
      else if (RUST_TYPE.test(word)) kind = "ty";
      else if (line[j] === "(") kind = "fn";
      else if (line[j] === "!") kind = "mc";
      out.push({ t: kind, v: word });
      i = j; continue;
    }
    // attribute
    if (c === "#" && line[i+1] === "[") {
      const end = line.indexOf("]", i+2);
      const stop = end === -1 ? line.length : end+1;
      out.push({ t: "at", v: line.slice(i, stop) });
      i = stop; continue;
    }
    // punct
    out.push({ t: "px", v: c });
    i++;
  }
  return out;
}
const TOK_STYLE = {
  cm: { color: "var(--fg-4)", fontStyle: "italic" },
  st: { color: "var(--good)" },
  nm: { color: "var(--warn)" },
  kw: { color: "var(--accent)", fontWeight: 600 },
  ty: { color: "var(--fg)" },
  fn: { color: "var(--fg)", fontWeight: 500 },
  mc: { color: "var(--accent)" },
  lf: { color: "var(--warn)", fontStyle: "italic" },
  at: { color: "var(--fg-3)" },
  id: { color: "var(--fg-2)" },
  px: { color: "var(--fg-3)" },
};

function CodeLine({ text }) {
  const tokens = useMemo(() => tokenizeRust(text), [text]);
  return (
    <>
      {tokens.map((tk, idx) => (
        <span key={idx} style={TOK_STYLE[tk.t]}>{tk.v}</span>
      ))}
    </>
  );
}

/* ───────────── sample rust file ───────────── */
const APERTURE_SRC = `// FIG. 02 — adaptive concurrency limiter
//   ref: papers/aperture-2024.pdf — § 3.1
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Aperture widens or narrows the permit pool to keep
/// observed latency near the configured target.
pub struct Aperture {
    permits: Arc<Semaphore>,
    target: Duration,
    inflight: AtomicUsize,
}

impl Aperture {
    pub fn new(initial: usize, target: Duration) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(initial)),
            target,
            inflight: AtomicUsize::new(0),
        }
    }

    pub async fn acquire(&self) -> Permit<'_> {
        let lease = self.permits.clone()
            .acquire_owned()
            .await
            .expect("aperture closed");
        self.inflight.fetch_add(1, Ordering::Relaxed);
        Permit { lease, started: Instant::now(), parent: self }
    }

    fn record(&self, elapsed: Duration) {
        let ratio = elapsed.as_secs_f64() / self.target.as_secs_f64();
        if ratio > 1.2 { self.shrink(); }
        else if ratio < 0.6 { self.widen(); }
    }
}`;

const APERTURE_LINES = APERTURE_SRC.split("\n");

/* logic structure: scope blocks with branches — for semantic minimap */
const SEMANTIC_BLOCKS = [
  { kind: "use",   from: 3,  to: 6,  label: "use ×4" },
  { kind: "doc",   from: 8,  to: 9,  label: "/// doc" },
  { kind: "type",  from: 10, to: 14, label: "struct Aperture",  branches: [] },
  { kind: "impl",  from: 16, to: 37, label: "impl Aperture",
    branches: [
      { name: "new",     from: 17, to: 23, kind: "fn" },
      { name: "acquire", from: 25, to: 31, kind: "fn async" },
      { name: "record",  from: 33, to: 36, kind: "fn",
        children: [
          { name: "if ratio > 1.2", from: 35, to: 35 },
          { name: "else if ratio < 0.6", from: 36, to: 36 },
        ]
      },
    ]
  },
];

/* ───────────── TopBar ───────────── */
function TopBar({ screen, setScreen, theme, toggleTheme, themeLabel, density, setDensity, scrubT, setScrubT, scrubbing, setScrubbing, showScrubber }) {
  return (
    <div style={{
      height: 36, display: "grid",
      gridTemplateColumns: "260px 1fr 260px",
      alignItems: "center", borderBottom: "1px solid var(--rule)",
      background: "var(--bg)", flexShrink: 0, position: "relative", zIndex: 30,
    }}>
      {/* brand */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, paddingLeft: 14 }}>
        <Glyph />
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span style={{ fontWeight: 700, letterSpacing: "0.18em", fontSize: 12, fontFamily: "var(--font-display)" }}>EDEN</span>
          <span className="label">v0.1.0 · α</span>
        </div>
      </div>

      {/* center — breadcrumb + scrubber */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 18, minWidth: 0 }}>
        <Breadcrumb parts={["hyperion","crates","aperture","src","lib.rs"]} />
        {showScrubber && (
          <TimeScrubber t={scrubT} setT={setScrubT} scrubbing={scrubbing} setScrubbing={setScrubbing} />
        )}
      </div>

      {/* right — chrome */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "flex-end", gap: 4, paddingRight: 10 }}>
        <ChromeBtn onClick={() => setDensity(density === "focus" ? "work" : "focus")}
          title={density === "focus" ? "Focus mode (⌘.)" : "Work mode"}>
          {density === "focus" ? "● FOCUS" : "○ WORK"}
        </ChromeBtn>
        <ChromeBtn onClick={toggleTheme} title="Cycle theme (next)">
          ◐ {themeLabel || "DARK"}
        </ChromeBtn>
      </div>
    </div>
  );
}

function Glyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" style={{ display: "block" }}>
      <rect x="0.5" y="0.5" width="17" height="17" stroke="currentColor" />
      <path d="M3 9 L9 3 L15 9 L9 15 Z" stroke="var(--accent)" strokeWidth="1.2" />
      <circle cx="9" cy="9" r="1.4" fill="var(--accent)" />
    </svg>
  );
}

function Breadcrumb({ parts }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: "var(--fg-3)" }}>
      {parts.map((p, i) => (
        <React.Fragment key={i}>
          {i > 0 && <span style={{ color: "var(--fg-4)" }}>/</span>}
          <span style={{ color: i === parts.length - 1 ? "var(--fg)" : "var(--fg-3)" }}>{p}</span>
        </React.Fragment>
      ))}
    </div>
  );
}

function ChromeBtn({ children, onClick, active, title }) {
  return (
    <button onClick={onClick} title={title} style={{
      background: active ? "var(--bg-2)" : "transparent",
      color: active ? "var(--fg)" : "var(--fg-3)",
      border: "1px solid transparent",
      padding: "5px 9px", fontFamily: "var(--mono)", fontSize: 10,
      letterSpacing: "0.14em", textTransform: "uppercase", cursor: "pointer",
      transition: "all 120ms var(--ease)",
    }}
    onMouseEnter={e => { e.currentTarget.style.color = "var(--fg)"; }}
    onMouseLeave={e => { if (!active) e.currentTarget.style.color = "var(--fg-3)"; }}
    >
      {children}
    </button>
  );
}

/* ───────────── Time scrubber ───────────── */
function TimeScrubber({ t, setT, scrubbing, setScrubbing }) {
  const ref = useRef(null);
  const ticks = useMemo(() => Array.from({length: 56}, (_,i) => i), []);
  // commit markers
  const commits = [4, 11, 19, 28, 38, 47, 53];

  const onDown = (e) => {
    setScrubbing(true);
    update(e);
    const move = (ev) => update(ev);
    const up = () => { setScrubbing(false); window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  const update = (e) => {
    const rect = ref.current.getBoundingClientRect();
    const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    setT(pct);
  };

  return (
    <div
      ref={ref}
      onPointerDown={onDown}
      style={{
        position: "relative", width: 280, height: 22,
        border: "1px solid var(--rule)", background: "var(--bg-elev)",
        cursor: "ew-resize", userSelect: "none",
      }}
      title="Drag to scrub through time"
    >
      {/* ticks */}
      <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "flex-end" }}>
        {ticks.map(i => {
          const isHr = i % 7 === 0;
          return <div key={i} style={{
            flex: 1, height: isHr ? 10 : 4,
            borderRight: i < 55 ? "1px solid var(--rule-2)" : "none",
            background: "transparent",
          }} />;
        })}
      </div>
      {/* commit dots */}
      {commits.map((c, i) => (
        <div key={i} style={{
          position: "absolute", top: 3, left: `${(c/56)*100}%`,
          width: 4, height: 4, background: "var(--accent)",
          transform: "translateX(-50%)",
        }} />
      ))}
      {/* playhead */}
      <div style={{
        position: "absolute", top: -2, bottom: -2, left: `${t*100}%`,
        width: 2, background: scrubbing ? "var(--accent)" : "var(--fg)",
        transform: "translateX(-50%)",
        boxShadow: scrubbing ? "0 0 10px var(--accent-glow)" : "none",
        transition: scrubbing ? "none" : "background 120ms var(--ease)",
      }} />
      {/* label */}
      <div style={{
        position: "absolute", top: -16, left: `${t*100}%`,
        transform: "translateX(-50%)",
        fontSize: 9, letterSpacing: "0.1em", color: "var(--fg-2)",
        whiteSpace: "nowrap", pointerEvents: "none",
      }}>
        {scrubbing ? `T-${Math.round((1-t)*47)}m` : "NOW"}
      </div>
      <div style={{
        position: "absolute", left: 6, top: 4, fontSize: 9,
        color: "var(--fg-4)", letterSpacing: "0.1em", pointerEvents: "none",
      }}>
        ⟲ TIME
      </div>
    </div>
  );
}

/* ───────────── Left rail ───────────── */
const RAIL_ITEMS = [
  { id: "editor",   glyph: "≡",  label: "Files" },
  { id: "spatial",  glyph: "◇",  label: "Canvas" },
  { id: "palette",  glyph: "⌘",  label: "Search" },
  { id: "debug",    glyph: "△",  label: "Debug" },
  { id: "terminal", glyph: "›_", label: "Term" },
  { id: "ai",       glyph: "✱",  label: "Pair" },
  { id: "settings", glyph: "⚙",  label: "Cfg",  bottom: true },
];

function LeftRail({ screen, setScreen }) {
  return (
    <div style={{
      width: 48, flexShrink: 0, display: "flex", flexDirection: "column",
      borderRight: "1px solid var(--rule)", background: "var(--bg)",
      paddingTop: 6, paddingBottom: 6,
    }}>
      {RAIL_ITEMS.filter(r => !r.bottom).map(item => (
        <RailBtn key={item.id} item={item} active={screen === item.id} onClick={() => setScreen(item.id)} />
      ))}
      <div style={{ flex: 1 }} />
      {RAIL_ITEMS.filter(r => r.bottom).map(item => (
        <RailBtn key={item.id} item={item} active={screen === item.id} onClick={() => setScreen(item.id)} />
      ))}
    </div>
  );
}

function RailBtn({ item, active, onClick }) {
  const [hover, setHover] = useState(false);
  return (
    <button onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      title={item.label}
      style={{
        position: "relative",
        width: 48, height: 44, background: "transparent", border: "none",
        color: active ? "var(--fg)" : "var(--fg-3)",
        fontSize: 14, cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center",
        transition: "color 120ms var(--ease)",
      }}>
      {active && <div style={{
        position: "absolute", left: 0, top: 6, bottom: 6, width: 2,
        background: "var(--accent)",
      }} />}
      <span style={{ filter: active ? "none" : "none" }}>{item.glyph}</span>
      {hover && (
        <div style={{
          position: "absolute", left: 52, top: "50%", transform: "translateY(-50%)",
          background: "var(--bg-elev)", border: "1px solid var(--rule)",
          padding: "3px 8px", fontSize: 10, letterSpacing: "0.14em",
          textTransform: "uppercase", color: "var(--fg-2)", whiteSpace: "nowrap",
          pointerEvents: "none", zIndex: 50, animation: "fadeIn 120ms var(--ease)",
        }}>{item.label}</div>
      )}
    </button>
  );
}

/* ───────────── File tree ───────────── */
const FILE_TREE = [
  { type: "dir", name: "hyperion", open: true, children: [
    { type: "dir", name: "crates", open: true, children: [
      { type: "dir", name: "aperture", open: true, children: [
        { type: "dir", name: "src", open: true, children: [
          { type: "file", name: "lib.rs", active: true, status: "M" },
          { type: "file", name: "permit.rs", status: null },
          { type: "file", name: "metrics.rs", status: "M" },
          { type: "file", name: "tests.rs", status: null },
        ]},
        { type: "file", name: "Cargo.toml", status: null },
      ]},
      { type: "dir", name: "ingest", children: [] },
      { type: "dir", name: "store", children: [] },
    ]},
    { type: "file", name: "Cargo.lock", status: null },
    { type: "file", name: "README.md", status: "?" },
  ]}
];

function FileTree({ density }) {
  if (density === "focus") return null;
  return (
    <aside style={{
      width: 240, flexShrink: 0, borderRight: "1px solid var(--rule)",
      background: "var(--bg)", display: "flex", flexDirection: "column",
      overflow: "hidden",
    }}>
      <div style={{
        padding: "10px 14px 8px", display: "flex", alignItems: "baseline",
        justifyContent: "space-between", borderBottom: "1px solid var(--rule-2)",
      }}>
        <span className="label-strong">FIG. 01 — Workspace</span>
        <span className="label num">04 / 47</span>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: "6px 0" }}>
        {FILE_TREE.map((n, i) => <TreeNode key={i} node={n} depth={0} />)}
      </div>
      <div style={{
        borderTop: "1px solid var(--rule)", padding: "8px 14px",
        display: "flex", flexDirection: "column", gap: 4,
      }}>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <span className="label">Branch</span>
          <span style={{ fontSize: 10, color: "var(--fg-2)" }}>main <span style={{ color: "var(--accent)" }}>↑2</span></span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <span className="label">Rust</span>
          <span style={{ fontSize: 10, color: "var(--fg-2)" }}>1.78 · stable</span>
        </div>
      </div>
    </aside>
  );
}

function TreeNode({ node, depth }) {
  const [open, setOpen] = useState(node.open ?? false);
  if (node.type === "dir") {
    return (
      <div>
        <div onClick={() => setOpen(!open)} style={{
          padding: `3px ${14 + depth*12}px`, fontSize: 11.5,
          color: "var(--fg-2)", cursor: "pointer", userSelect: "none",
          display: "flex", alignItems: "center", gap: 6,
        }}>
          <span style={{ color: "var(--fg-4)", fontSize: 9, width: 8 }}>{open ? "▾" : "▸"}</span>
          <span style={{ fontWeight: 500 }}>{node.name}</span>
        </div>
        {open && node.children?.map((c, i) => <TreeNode key={i} node={c} depth={depth+1} />)}
      </div>
    );
  }
  return (
    <div style={{
      padding: `3px ${14 + depth*12 + 14}px`, fontSize: 11.5,
      color: node.active ? "var(--fg)" : "var(--fg-2)",
      background: node.active ? "var(--bg-2)" : "transparent",
      borderLeft: node.active ? "2px solid var(--accent)" : "2px solid transparent",
      marginLeft: 0, cursor: "pointer", display: "flex", justifyContent: "space-between",
    }}>
      <span>{node.name}</span>
      {node.status && (
        <span style={{
          fontSize: 9, letterSpacing: "0.1em",
          color: node.status === "M" ? "var(--warn)" : node.status === "?" ? "var(--fg-3)" : "var(--good)",
        }}>{node.status}</span>
      )}
    </div>
  );
}

/* ───────────── Editor pane ───────────── */
function EditorPane({ density, features, scrubT, scrubbing }) {
  const focusLine = 27; // acquire() body
  // Determine which lines are "diff" — animated as tactile change
  const [diffSeed, setDiffSeed] = useState(0);

  // Replay tactile diff on click of "Apply suggestion"
  const replayDiff = () => setDiffSeed(s => s+1);

  // When scrubbing back, simulate older content state
  const olderState = scrubT < 0.85;
  const dofOn = features.focus && density === "focus";

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0, background: "var(--bg)" }}>
      {/* tabs */}
      <Tabs />

      {/* code body */}
      <div style={{ flex: 1, display: "flex", minHeight: 0, position: "relative" }}>
        <CodeBody
          lines={APERTURE_LINES}
          focusLine={focusLine}
          diffSeed={diffSeed}
          dofOn={dofOn}
          olderState={olderState}
          features={features}
          density={density}
          onApply={replayDiff}
        />
        {features.minimap && density !== "focus" && (
          <SemanticMinimap blocks={SEMANTIC_BLOCKS} focusLine={focusLine} />
        )}
      </div>

      {/* status bar */}
      <StatusBar />
    </div>
  );
}

function Tabs() {
  const tabs = [
    { name: "lib.rs", path: "aperture/src", active: true, dirty: true },
    { name: "permit.rs", path: "aperture/src" },
    { name: "metrics.rs", path: "aperture/src", dirty: true },
    { name: "Cargo.toml", path: "aperture" },
  ];
  return (
    <div style={{
      display: "flex", height: 32, borderBottom: "1px solid var(--rule)",
      background: "var(--bg)", flexShrink: 0,
    }}>
      {tabs.map((t, i) => (
        <div key={i} style={{
          display: "flex", alignItems: "center", gap: 8,
          padding: "0 14px", fontSize: 11,
          color: t.active ? "var(--fg)" : "var(--fg-3)",
          background: t.active ? "var(--bg-elev)" : "transparent",
          borderRight: "1px solid var(--rule)",
          borderTop: t.active ? "1px solid var(--accent)" : "1px solid transparent",
          marginTop: t.active ? -1 : 0,
          cursor: "pointer", position: "relative",
        }}>
          <span style={{ fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.1em" }}>{String(i+1).padStart(2,"0")}</span>
          <span>{t.name}</span>
          {t.dirty && <span style={{ width: 5, height: 5, background: "var(--accent)", borderRadius: 0 }} />}
        </div>
      ))}
      <div style={{ flex: 1 }} />
      <div style={{
        padding: "0 14px", display: "flex", alignItems: "center",
        fontSize: 9, letterSpacing: "0.14em", color: "var(--fg-4)",
        textTransform: "uppercase",
      }}>
        TAB · 04
      </div>
    </div>
  );
}

/* The actual code surface */
function CodeBody({ lines, focusLine, diffSeed, dofOn, olderState, features, density, onApply }) {
  // diff-marked lines (changes shown as fresh)
  const diffLines = new Set([33, 34, 35, 36]); // record() body — was added recently
  // older state shows the previous version of these lines:
  const olderOverrides = {
    33: "    fn record(&self, _elapsed: Duration) {",
    34: "        // TODO: implement adaptive scaling",
    35: "        unimplemented!()",
    36: "    }",
  };

  // Compute the scope group that "owns" the focus line for DOF
  const focusScopeFrom = 25, focusScopeTo = 31;

  return (
    <div style={{
      flex: 1, display: "flex", position: "relative",
      overflow: "hidden", minWidth: 0,
      background: "var(--bg)",
      fontFamily: "var(--mono)",
    }}>
      {/* breathing background grid */}
      {features.breath && (
        <div style={{
          position: "absolute", inset: 0, pointerEvents: "none",
          backgroundImage: "linear-gradient(var(--rule-2) 1px, transparent 1px), linear-gradient(90deg, var(--rule-2) 1px, transparent 1px)",
          backgroundSize: "24px 24px",
          opacity: 0.25, animation: "drift 8s ease-in-out infinite",
        }} />
      )}

      <div style={{ flex: 1, overflow: "auto", paddingTop: 16, paddingBottom: 80, position: "relative" }}>
        {lines.map((raw, i) => {
          const ln = i + 1;
          const text = olderState && olderOverrides[ln] ? olderOverrides[ln] : raw;
          const isFocus = ln === focusLine;
          const inFocusScope = ln >= focusScopeFrom && ln <= focusScopeTo;
          const isDiff = !olderState && diffLines.has(ln);

          // depth-of-field blur for non-focus scopes
          let blur = 0, opacity = 1;
          if (dofOn && !inFocusScope) {
            const dist = Math.min(Math.abs(ln - focusScopeFrom), Math.abs(ln - focusScopeTo));
            blur = Math.min(3, dist * 0.3);
            opacity = Math.max(0.3, 1 - dist * 0.05);
          }

          return (
            <div key={`${ln}-${diffSeed}`} style={{
              display: "flex", alignItems: "stretch", position: "relative",
              minHeight: 19,
              animation: isDiff ? `diffSettle 700ms var(--ease-spring) both` : undefined,
              filter: blur ? `blur(${blur}px)` : undefined,
              opacity,
              transition: "filter 320ms var(--ease), opacity 320ms var(--ease)",
            }}>
              {/* gutter */}
              <div style={{
                width: 56, flexShrink: 0, textAlign: "right",
                paddingRight: 10, fontSize: 11, color: isFocus ? "var(--accent)" : "var(--fg-4)",
                userSelect: "none", paddingTop: 1, paddingBottom: 1,
                fontVariantNumeric: "tabular-nums",
              }}>
                {String(ln).padStart(3, " ")}
              </div>
              {/* change marker */}
              <div style={{
                width: 4, flexShrink: 0,
                background: isDiff ? "var(--accent)" : "transparent",
                animation: isDiff ? `diffGlow 1.4s var(--ease) both` : undefined,
              }} />
              {/* code */}
              <div style={{
                flex: 1, paddingLeft: 14, paddingRight: 20,
                fontSize: 12.5, lineHeight: "19px",
                color: "var(--fg)", whiteSpace: "pre",
                position: "relative",
                userSelect: "text",
              }}>
                <CodeLine text={text} />
                {isFocus && (
                  <span style={{
                    display: "inline-block", width: 2, height: 14,
                    background: "var(--accent)", marginLeft: 1,
                    transform: "translateY(2px)",
                    animation: "cursorBlink 1.05s steps(2) infinite",
                  }} />
                )}
                {/* AI ambient annotation */}
                {ln === 30 && features.ai && !olderState && (
                  <AIMargin />
                )}
              </div>
            </div>
          );
        })}

        {/* tactile diff suggestion control */}
        {!olderState && features.diff && (
          <div style={{
            position: "absolute", right: 18, top: 12,
            display: "flex", alignItems: "center", gap: 8,
          }}>
            <span className="label">CHANGES · 4 LINES</span>
            <button onClick={onApply} style={{
              fontFamily: "var(--mono)", fontSize: 10, letterSpacing: "0.14em",
              textTransform: "uppercase", background: "var(--accent)", color: "var(--bg)",
              border: "none", padding: "4px 10px", cursor: "pointer",
            }}>
              ↻ Replay diff
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

/* AI ambient annotation — lives in the margin, not a chat panel */
function AIMargin() {
  return (
    <div style={{
      position: "absolute", right: 24, top: -2,
      display: "flex", alignItems: "flex-start", gap: 10,
      animation: "fadeUp 380ms var(--ease) both",
      animationDelay: "600ms",
      maxWidth: 340,
    }}>
      <div style={{
        width: 60, height: 1, background: "var(--accent)",
        marginTop: 9, flexShrink: 0,
      }} />
      <div style={{
        background: "var(--bg-elev)", border: "1px solid var(--accent)",
        padding: "6px 10px 7px", minWidth: 220,
      }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 6, marginBottom: 3 }}>
          <span style={{ color: "var(--accent)", fontSize: 9, letterSpacing: "0.18em", fontWeight: 700 }}>✱ EDEN</span>
          <span className="label">margin · 0.4s ago</span>
        </div>
        <div style={{ fontSize: 11, color: "var(--fg-2)", lineHeight: 1.45 }}>
          <span style={{ color: "var(--fg)" }}>fetch_add</span> can wrap on overflow under load.
          Consider <span style={{ color: "var(--accent)" }}>checked_add</span> with a back-pressure path.
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
          <span style={{ fontSize: 9, color: "var(--fg-3)", letterSpacing: "0.14em" }}>↵ ACCEPT</span>
          <span style={{ fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.14em" }}>⎋ DISMISS</span>
        </div>
      </div>
    </div>
  );
}

/* Semantic minimap — shows logic flow, not lines */
function SemanticMinimap({ blocks, focusLine }) {
  return (
    <aside style={{
      width: 200, flexShrink: 0, borderLeft: "1px solid var(--rule)",
      background: "var(--bg)", display: "flex", flexDirection: "column",
    }}>
      <div style={{
        padding: "10px 14px 8px", display: "flex", alignItems: "baseline",
        justifyContent: "space-between", borderBottom: "1px solid var(--rule-2)",
      }}>
        <span className="label-strong">FIG. 03 — Logic</span>
        <span className="label">scope</span>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: "10px 12px", position: "relative" }}>
        {blocks.map((b, i) => <MinimapBlock key={i} block={b} focusLine={focusLine} />)}
        <div style={{
          marginTop: 16, paddingTop: 12, borderTop: "1px dashed var(--rule)",
        }}>
          <div className="label" style={{ marginBottom: 6 }}>complexity</div>
          <div style={{ display: "flex", gap: 2, alignItems: "flex-end", height: 30 }}>
            {[3, 5, 8, 4, 2, 6, 4, 9, 7, 3, 2, 4, 5].map((h, i) => (
              <div key={i} style={{
                flex: 1, height: `${h * 3}px`,
                background: h > 7 ? "var(--accent)" : "var(--fg-4)",
              }} />
            ))}
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 4 }}>
            <span className="label" style={{ fontSize: 9 }}>fn</span>
            <span className="label" style={{ fontSize: 9 }}>cyclomatic</span>
          </div>
        </div>
      </div>
    </aside>
  );
}

function MinimapBlock({ block, focusLine }) {
  const hot = focusLine >= block.from && focusLine <= block.to;
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{
        display: "flex", alignItems: "center", gap: 6, marginBottom: 4,
      }}>
        <div style={{
          width: 2, height: 10,
          background: hot ? "var(--accent)" : "var(--fg-4)",
        }} />
        <span style={{
          fontSize: 10, color: hot ? "var(--fg)" : "var(--fg-2)",
          fontWeight: hot ? 600 : 400,
        }}>{block.label}</span>
        <span style={{ marginLeft: "auto", fontSize: 9, color: "var(--fg-4)" }}>
          {block.from}–{block.to}
        </span>
      </div>
      {block.branches && (
        <div style={{ marginLeft: 10, borderLeft: "1px solid var(--rule)", paddingLeft: 8 }}>
          {block.branches.map((br, i) => {
            const isHot = focusLine >= br.from && focusLine <= br.to;
            return (
              <div key={i} style={{ marginTop: 4 }}>
                <div style={{
                  display: "flex", alignItems: "center", gap: 6,
                  fontSize: 10, color: isHot ? "var(--fg)" : "var(--fg-3)",
                }}>
                  <span style={{ color: "var(--fg-4)" }}>┐</span>
                  <span>{br.name}</span>
                  {isHot && <span style={{
                    marginLeft: "auto", color: "var(--accent)", fontSize: 9,
                  }}>● HERE</span>}
                </div>
                {br.children && (
                  <div style={{ marginLeft: 14, marginTop: 2 }}>
                    {br.children.map((c, j) => (
                      <div key={j} style={{
                        fontSize: 9, color: "var(--fg-3)",
                        display: "flex", gap: 6, padding: "1px 0",
                      }}>
                        <span style={{ color: "var(--fg-4)" }}>↘</span>
                        <span>{c.name}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* ───────────── Status bar ───────────── */
function StatusBar() {
  return (
    <div style={{
      height: 26, borderTop: "1px solid var(--rule)",
      display: "flex", alignItems: "center", fontSize: 10,
      letterSpacing: "0.1em", color: "var(--fg-3)", flexShrink: 0,
      background: "var(--bg)",
    }}>
      <Pill color="var(--good)">● RUST-ANALYZER · IDLE</Pill>
      <Sep />
      <Pill>main.rs</Pill>
      <Sep />
      <Pill>LN 27 · COL 24</Pill>
      <Sep />
      <Pill>UTF-8 · LF</Pill>
      <Sep />
      <Pill>CARGO 1.78</Pill>
      <div style={{ flex: 1 }} />
      <Pill>0 ERR · 2 WARN</Pill>
      <Sep />
      <Pill color="var(--accent)">⟲ AUTOSAVED 4s</Pill>
      <Sep />
      <Pill>git: main <span style={{ color: "var(--accent)" }}>↑2</span></Pill>
    </div>
  );
}
function Sep() {
  return <div style={{ width: 1, height: 12, background: "var(--rule)" }} />;
}
function Pill({ children, color }) {
  return (
    <div style={{
      padding: "0 12px", color: color || "var(--fg-3)",
      textTransform: "uppercase", whiteSpace: "nowrap",
    }}>{children}</div>
  );
}

/* ───────────── Main editor screen ───────────── */
function MainEditor({ density, features, scrubT, scrubbing }) {
  return (
    <div style={{ display: "flex", flex: 1, minHeight: 0, background: "var(--bg)" }}>
      <FileTree density={density} />
      <EditorPane density={density} features={features} scrubT={scrubT} scrubbing={scrubbing} />
    </div>
  );
}

Object.assign(window, {
  MainEditor, TopBar, LeftRail, Glyph, ChromeBtn, Breadcrumb,
  CodeLine, tokenizeRust, APERTURE_LINES, SEMANTIC_BLOCKS,
});
