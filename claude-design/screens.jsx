/* SCREENS.JSX — secondary screens: welcome, palette, terminal, debugger, settings, AI, onboarding, spatial canvas */

const { useState: useS, useEffect: useE, useRef: useR, useMemo: useM } = React;

/* ═════════════════════════════ WELCOME ═════════════════════════════ */
function WelcomeScreen({ onOpen }) {
  const recents = [
    { name: "hyperion",         path: "~/code/hyperion",      time: "4m",   loc: "184,302", lang: "rust" },
    { name: "aperture",         path: "~/work/aperture",      time: "1h",   loc: "12,847",  lang: "rust" },
    { name: "kinesis-protocol", path: "~/research/kinesis",   time: "yday", loc: "47,201",  lang: "rust" },
    { name: "vesper-cli",       path: "~/tools/vesper-cli",   time: "3d",   loc: "3,902",   lang: "rust" },
    { name: "obelisk-runtime",  path: "~/code/obelisk",       time: "1w",   loc: "92,114",  lang: "rust" },
  ];
  return (
    <div style={{
      flex: 1, display: "grid",
      gridTemplateColumns: "1.1fr 1fr",
      background: "var(--bg)", minHeight: 0, overflow: "hidden",
    }}>
      {/* left — masthead */}
      <div style={{
        padding: "56px 56px 36px", display: "flex", flexDirection: "column",
        justifyContent: "space-between", borderRight: "1px solid var(--rule)",
        position: "relative", overflow: "hidden",
      }}>
        <div>
          <div style={{ display: "flex", alignItems: "baseline", gap: 14, marginBottom: 8 }}>
            <span className="label">VOLUME 01 · ISSUE 01</span>
            <span className="label">MAY · 2026</span>
            <span className="label">v0.1.0 · α</span>
          </div>
          <div style={{ borderTop: "2px solid var(--fg)", marginBottom: 18 }} />
          <h1 style={{
            margin: 0, fontFamily: "var(--font-display)", fontWeight: 700,
            fontSize: 120, lineHeight: 0.86, letterSpacing: "var(--display-tracking)",
          }}>EDEN</h1>
          <div style={{
            marginTop: 14, fontSize: 13, color: "var(--fg-2)", maxWidth: 460,
            fontFamily: "var(--serif)", fontStyle: "italic", lineHeight: 1.35,
          }}>
            An editor for the new world — a Rust-native workspace where time
            is reversible, code is spatial, and your collaborator lives in the margin.
          </div>
          <div style={{
            marginTop: 22, display: "grid",
            gridTemplateColumns: "auto 1fr", gap: "6px 14px",
            fontSize: 11, color: "var(--fg-3)", maxWidth: 460,
          }}>
            <span className="label">RUNTIME</span><span style={{color:"var(--fg-2)"}}>Rust 1.78 · zero-GC · 60 fps target</span>
            <span className="label">PLATFORM</span><span style={{color:"var(--fg-2)"}}>macOS · linux · windows (preview)</span>
            <span className="label">PROTOCOL</span><span style={{color:"var(--fg-2)"}}>LSP · DAP · TreeSitter · MCP</span>
            <span className="label">LICENSE</span><span style={{color:"var(--fg-2)"}}>source-available · § eden.dev/license</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <BigBtn primary onClick={() => onOpen("editor")}>OPEN HYPERION ↗</BigBtn>
          <BigBtn onClick={() => onOpen("onboarding")}>FIRST RUN</BigBtn>
          <BigBtn onClick={() => onOpen("spatial")}>SPATIAL CANVAS</BigBtn>
        </div>
        <Crosshair />
      </div>

      {/* right — recent + colophon */}
      <div style={{ display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{
          padding: "56px 48px 14px",
          display: "flex", justifyContent: "space-between", alignItems: "baseline",
        }}>
          <span className="label-strong">FIG. 04 — RECENT WORKSPACES</span>
          <span className="label">05 / 12</span>
        </div>
        <div style={{ borderTop: "1px solid var(--fg)", margin: "0 48px" }} />
        <div style={{ flex: 1, overflow: "auto", padding: "0 48px" }}>
          {recents.map((r, i) => (
            <RecentRow key={i} idx={i} r={r} onClick={() => onOpen("editor")} />
          ))}
        </div>
        <div style={{
          borderTop: "1px solid var(--rule)", padding: "16px 48px",
          display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 10, fontSize: 10,
          color: "var(--fg-3)",
        }}>
          <Stat k="UPTIME" v="00:14:22" />
          <Stat k="MEM" v="184 MB" />
          <Stat k="FPS" v="60.0" />
          <Stat k="CRATES" v="47 indexed" />
          <Stat k="LSP" v="● ready" />
          <Stat k="EDEN" v="✱ ambient" />
        </div>
      </div>
    </div>
  );
}

function Crosshair() {
  return (
    <svg style={{ position: "absolute", bottom: 24, right: 24, opacity: 0.5 }} width="56" height="56" viewBox="0 0 56 56" fill="none">
      <circle cx="28" cy="28" r="14" stroke="var(--fg-3)" />
      <line x1="28" y1="0" x2="28" y2="20" stroke="var(--fg-3)" />
      <line x1="28" y1="36" x2="28" y2="56" stroke="var(--fg-3)" />
      <line x1="0" y1="28" x2="20" y2="28" stroke="var(--fg-3)" />
      <line x1="36" y1="28" x2="56" y2="28" stroke="var(--fg-3)" />
      <circle cx="28" cy="28" r="2" fill="var(--accent)" />
    </svg>
  );
}

function BigBtn({ children, onClick, primary }) {
  const [h, setH] = useS(false);
  return (
    <button onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        fontFamily: "var(--mono)", fontSize: 11, letterSpacing: "0.18em",
        padding: "12px 18px", cursor: "pointer",
        background: primary ? "var(--fg)" : "transparent",
        color: primary ? "var(--bg)" : "var(--fg)",
        border: primary ? "1px solid var(--fg)" : "1px solid var(--fg)",
        transform: h ? "translate(-1px,-1px)" : "translate(0,0)",
        boxShadow: h ? "2px 2px 0 var(--fg)" : "0 0 0 transparent",
        transition: "all 140ms var(--ease)",
      }}>
      {children}
    </button>
  );
}

function RecentRow({ r, idx, onClick }) {
  const [h, setH] = useS(false);
  return (
    <div onClick={onClick} onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)} style={{
      display: "grid", gridTemplateColumns: "32px 1fr auto auto",
      gap: 14, padding: "14px 0", borderBottom: "1px solid var(--rule)",
      cursor: "pointer", alignItems: "center",
      background: h ? "var(--bg-2)" : "transparent",
      margin: "0 -16px", padding: "14px 16px",
      transition: "background 100ms var(--ease)",
    }}>
      <span style={{ fontSize: 10, color: "var(--fg-4)", letterSpacing: "0.1em" }}>{String(idx+1).padStart(2,"0")}</span>
      <div>
        <div style={{ fontSize: 16, fontWeight: 500, color: "var(--fg)" }}>{r.name}</div>
        <div style={{ fontSize: 10, color: "var(--fg-3)", marginTop: 2, letterSpacing: "0.06em" }}>{r.path}</div>
      </div>
      <div style={{ fontSize: 10, color: "var(--fg-3)", textAlign: "right", letterSpacing: "0.06em" }}>
        <div>{r.loc} loc</div>
        <div style={{ color: "var(--fg-4)" }}>{r.lang}</div>
      </div>
      <div style={{ fontSize: 10, color: "var(--accent)", letterSpacing: "0.1em" }}>· {r.time}</div>
    </div>
  );
}

function Stat({ k, v }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", borderBottom: "1px solid var(--rule-2)", padding: "4px 0" }}>
      <span style={{ letterSpacing: "0.14em", color: "var(--fg-4)" }}>{k}</span>
      <span style={{ color: "var(--fg-2)" }}>{v}</span>
    </div>
  );
}

/* ═════════════════════════════ COMMAND PALETTE ═════════════════════════════ */
function CommandPalette({ onClose }) {
  const [q, setQ] = useS("");
  const inputRef = useR(null);
  useE(() => { inputRef.current?.focus(); }, []);

  const all = [
    { kind: "file",  icon: "≡", name: "lib.rs",       sub: "crates/aperture/src", key: "↵" },
    { kind: "file",  icon: "≡", name: "permit.rs",    sub: "crates/aperture/src", key: "↵" },
    { kind: "sym",   icon: "ƒ", name: "Aperture::acquire", sub: "lib.rs · 25",   key: "↵" },
    { kind: "sym",   icon: "ƒ", name: "Aperture::record",  sub: "lib.rs · 33",   key: "↵" },
    { kind: "sym",   icon: "𝒯", name: "struct Aperture",   sub: "lib.rs · 10",   key: "↵" },
    { kind: "cmd",   icon: "⌘", name: "Toggle Focus Mode", sub: "view",          key: "⌘ ." },
    { kind: "cmd",   icon: "⌘", name: "Scrub To Commit…",  sub: "time",          key: "⌘ ⇧ T" },
    { kind: "cmd",   icon: "⌘", name: "Open Spatial Canvas", sub: "view",        key: "⌘ ⇧ C" },
    { kind: "ai",    icon: "✱", name: "Ask EDEN: 'why does this allocate?'", sub: "ambient pair", key: "⌥ ↵" },
    { kind: "ai",    icon: "✱", name: "Refactor: extract retry policy",      sub: "ambient pair", key: "⌥ ↵" },
  ];

  const filtered = q
    ? all.filter(x => (x.name + " " + x.sub).toLowerCase().includes(q.toLowerCase()))
    : all;

  return (
    <div style={{
      position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
      display: "flex", alignItems: "flex-start", justifyContent: "center",
      paddingTop: 110, zIndex: 200,
      animation: "fadeIn 140ms var(--ease)",
    }} onClick={onClose}>
      <div onClick={e => e.stopPropagation()} style={{
        width: 680, background: "var(--bg-elev)",
        border: "1px solid var(--fg)",
        boxShadow: "10px 10px 0 var(--bg-2), 11px 11px 0 var(--accent)",
        animation: "fadeUp 200ms var(--ease)",
      }}>
        {/* header */}
        <div style={{
          display: "flex", alignItems: "center", borderBottom: "1px solid var(--rule)",
        }}>
          <span style={{
            padding: "0 14px", fontSize: 10, letterSpacing: "0.18em",
            color: "var(--accent)", fontWeight: 700,
          }}>⌘</span>
          <input ref={inputRef} value={q} onChange={e => setQ(e.target.value)} placeholder="search files · symbols · ask EDEN…"
            style={{
              flex: 1, background: "transparent", border: "none", outline: "none",
              padding: "14px 0", fontFamily: "var(--mono)", fontSize: 14, color: "var(--fg)",
            }} />
          <span style={{
            padding: "0 14px", fontSize: 9, letterSpacing: "0.14em",
            color: "var(--fg-4)",
          }}>{filtered.length} · ESC TO DISMISS</span>
        </div>

        {/* breadcrumb of intent */}
        <div style={{
          padding: "6px 16px", display: "flex", gap: 14, fontSize: 9,
          letterSpacing: "0.16em", color: "var(--fg-3)",
          borderBottom: "1px solid var(--rule-2)",
        }}>
          <span style={{ color: "var(--fg)" }}>● ALL</span>
          <span>FILES</span>
          <span>SYMBOLS</span>
          <span>COMMANDS</span>
          <span style={{ color: "var(--accent)" }}>✱ ASK EDEN</span>
        </div>

        {/* results */}
        <div style={{ maxHeight: 380, overflow: "auto" }}>
          {filtered.map((it, i) => (
            <div key={i} style={{
              display: "grid", gridTemplateColumns: "32px 1fr auto",
              alignItems: "center", padding: "9px 14px",
              fontSize: 12,
              background: i === 0 ? "var(--bg-2)" : "transparent",
              borderLeft: i === 0 ? "2px solid var(--accent)" : "2px solid transparent",
              borderBottom: "1px solid var(--rule-2)",
              cursor: "pointer",
            }}>
              <span style={{
                color: it.kind === "ai" ? "var(--accent)" : "var(--fg-3)",
                fontSize: 13, textAlign: "center",
              }}>{it.icon}</span>
              <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
                <span style={{ color: "var(--fg)", fontWeight: i === 0 ? 500 : 400 }}>
                  {it.name}
                </span>
                <span style={{ fontSize: 10, color: "var(--fg-4)" }}>{it.sub}</span>
              </div>
              <span style={{
                fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.14em",
                border: "1px solid var(--rule)", padding: "2px 6px",
              }}>{it.key}</span>
            </div>
          ))}
        </div>

        <div style={{
          padding: "6px 14px", display: "flex", justifyContent: "space-between",
          fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.14em",
          borderTop: "1px solid var(--rule)",
        }}>
          <span>↑↓ NAVIGATE · ↵ OPEN · ⌥↵ ASK</span>
          <span style={{ color: "var(--accent)" }}>● 47ms · 4,802 INDEXED</span>
        </div>
      </div>
    </div>
  );
}

/* ═════════════════════════════ TERMINAL ═════════════════════════════ */
function TerminalScreen() {
  const lines = [
    { kind:"prompt", v:"hyperion @ aperture", path:"src", cmd:"cargo test --lib" },
    { kind:"out",  v:"   Compiling aperture v0.3.1 (~/work/aperture)" },
    { kind:"out",  v:"    Finished test [optimized + debuginfo] in 2.41s" },
    { kind:"out",  v:"     Running unittests src/lib.rs (target/debug/deps/aperture-7a3c4)" },
    { kind:"blank" },
    { kind:"out",  v:"running 9 tests" },
    { kind:"ok",   v:"test tests::widens_under_low_latency ........ ok" },
    { kind:"ok",   v:"test tests::shrinks_above_target ............ ok" },
    { kind:"ok",   v:"test tests::permit_release_on_drop .......... ok" },
    { kind:"ok",   v:"test tests::inflight_counter_increments ..... ok" },
    { kind:"warn", v:"test tests::respects_max_pool ............... ignored (TODO: § 3.2)" },
    { kind:"ok",   v:"test tests::concurrent_acquires_serialize ... ok" },
    { kind:"fail", v:"test tests::record_under_overflow ........... FAILED" },
    { kind:"ok",   v:"test tests::aperture_closes_cleanly ......... ok" },
    { kind:"ok",   v:"test tests::serde_roundtrip ................. ok" },
    { kind:"blank" },
    { kind:"out",  v:"failures:" },
    { kind:"out",  v:"  ---- tests::record_under_overflow stdout ----" },
    { kind:"err",  v:"  thread 'tests::record_under_overflow' panicked at:" },
    { kind:"err",  v:"  attempt to add with overflow, src/lib.rs:30" },
    { kind:"blank" },
    { kind:"out",  v:"test result: FAILED. 7 passed; 1 failed; 1 ignored" },
    { kind:"blank" },
    { kind:"prompt", v:"hyperion @ aperture", path:"src", cmd:"", cursor: true },
  ];

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", background: "var(--bg)", minHeight: 0 }}>
      <PaneHeader fig="07" title="Terminal" right="bash · 80×24 · session 3 of 3" />
      <div style={{
        flex: 1, overflow: "auto", padding: "16px 24px",
        fontSize: 12.5, lineHeight: 1.55, fontFamily: "var(--mono)",
      }}>
        {lines.map((l, i) => <TermLine key={i} l={l} />)}
      </div>

      <div style={{
        borderTop: "1px solid var(--rule)", padding: "8px 16px",
        display: "flex", gap: 18, fontSize: 9, letterSpacing: "0.14em",
        color: "var(--fg-3)",
      }}>
        <span>EXIT 101</span>
        <span>2.41 SEC</span>
        <span style={{ color: "var(--bad)" }}>● 1 FAIL</span>
        <span style={{ color: "var(--warn)" }}>● 1 IGNORED</span>
        <span style={{ color: "var(--good)" }}>● 7 PASS</span>
        <div style={{ flex: 1 }} />
        <span style={{ color: "var(--accent)" }}>✱ EDEN can patch this — ⌥↵</span>
      </div>
    </div>
  );
}

function TermLine({ l }) {
  if (l.kind === "blank") return <div style={{ height: 8 }} />;
  if (l.kind === "prompt") {
    return (
      <div style={{ display: "flex", gap: 8, color: "var(--fg-2)" }}>
        <span style={{ color: "var(--accent)" }}>›</span>
        <span style={{ color: "var(--fg-3)" }}>{l.v}</span>
        <span style={{ color: "var(--fg-4)" }}>/{l.path}</span>
        <span style={{ color: "var(--fg)" }}>{l.cmd}</span>
        {l.cursor && (
          <span style={{
            display: "inline-block", width: 8, height: 14,
            background: "var(--accent)", animation: "cursorBlink 1.05s steps(2) infinite",
            verticalAlign: "text-bottom",
          }} />
        )}
      </div>
    );
  }
  const colors = {
    ok:   "var(--fg-2)",
    warn: "var(--warn)",
    fail: "var(--bad)",
    err:  "var(--bad)",
    out:  "var(--fg-3)",
  };
  const prefix = { ok:"  ", warn:"  ", fail:"  ", err:"  ", out:"" }[l.kind] || "";
  return (
    <div style={{ color: colors[l.kind] || "var(--fg-3)", whiteSpace: "pre" }}>
      {prefix}{l.v}
    </div>
  );
}

/* ═════════════════════════════ DEBUGGER ═════════════════════════════ */
function DebuggerScreen() {
  const stack = [
    { idx: 0, fn: "Aperture::record", file: "lib.rs:35", here: true },
    { idx: 1, fn: "Permit::drop",     file: "permit.rs:42" },
    { idx: 2, fn: "tokio::runtime::task::poll", file: "<runtime>" },
    { idx: 3, fn: "tokio::runtime::scheduler::worker::run", file: "<runtime>" },
    { idx: 4, fn: "main::{closure}",  file: "main.rs:12" },
  ];
  const vars = [
    { name: "self.target",   ty: "Duration",     val: "100ms" },
    { name: "self.inflight", ty: "AtomicUsize",  val: "usize::MAX" },
    { name: "elapsed",       ty: "Duration",     val: "42ms" },
    { name: "ratio",         ty: "f64",          val: "0.42" },
    { name: "self.permits",  ty: "Arc<Semaphore>", val: "Semaphore { permits: 12 }" },
  ];
  return (
    <div style={{
      flex: 1, display: "grid",
      gridTemplateColumns: "260px 1fr 320px",
      background: "var(--bg)", minHeight: 0, overflow: "hidden",
    }}>
      {/* stack */}
      <aside style={{ borderRight: "1px solid var(--rule)", display: "flex", flexDirection: "column" }}>
        <PaneHeader fig="06A" title="Call Stack" right="5 frames" />
        <div style={{ flex: 1, overflow: "auto" }}>
          {stack.map((f, i) => (
            <div key={i} style={{
              padding: "10px 14px", borderBottom: "1px solid var(--rule-2)",
              background: f.here ? "var(--bg-2)" : "transparent",
              borderLeft: f.here ? "2px solid var(--accent)" : "2px solid transparent",
              display: "flex", flexDirection: "column", gap: 2, cursor: "pointer",
            }}>
              <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
                <span style={{ fontSize: 9, color: "var(--fg-4)" }}>#{f.idx}</span>
                <span style={{ fontSize: 11.5, color: f.here ? "var(--fg)" : "var(--fg-2)" }}>{f.fn}</span>
              </div>
              <span style={{ fontSize: 10, color: "var(--fg-3)", marginLeft: 16 }}>{f.file}</span>
            </div>
          ))}
        </div>
      </aside>

      {/* center — source + breakpoint */}
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
        <PaneHeader fig="06B" title="Frame 0 · Aperture::record" right="⌃ PAUSED · panic · attempt to add with overflow" rightAccent />
        <div style={{ flex: 1, overflow: "auto", padding: "14px 0", fontSize: 12.5, lineHeight: "20px", fontFamily: "var(--mono)" }}>
          {[
            { l: 33, code: "    fn record(&self, elapsed: Duration) {" },
            { l: 34, code: "        let ratio = elapsed.as_secs_f64() / self.target.as_secs_f64();" },
            { l: 35, code: "        if ratio > 1.2 { self.shrink(); }", here: true, panic: true },
            { l: 36, code: "        else if ratio < 0.6 { self.widen(); }" },
            { l: 37, code: "    }" },
            { l: 38, code: "" },
            { l: 39, code: "    fn shrink(&self) {" },
            { l: 40, code: "        self.permits.forget_permits(1);" },
            { l: 41, code: "    }" },
            { l: 42, code: "" },
            { l: 43, code: "    fn widen(&self) {" },
            { l: 44, code: "        self.permits.add_permits(1);" },
            { l: 45, code: "        self.inflight.fetch_add(1, Ordering::Relaxed);" },
            { l: 46, code: "    }" },
          ].map((row, i) => (
            <div key={i} style={{
              display: "flex", alignItems: "center", minHeight: 20,
              background: row.here ? "var(--bg-2)" : "transparent",
              borderLeft: row.here ? "2px solid var(--accent)" : "2px solid transparent",
              position: "relative",
            }}>
              <span style={{ width: 14, textAlign: "center", color: "var(--bad)", fontSize: 12 }}>
                {row.l === 35 ? "●" : ""}
              </span>
              <span style={{ width: 36, textAlign: "right", paddingRight: 10, fontSize: 11, color: row.here ? "var(--fg)" : "var(--fg-4)" }}>{row.l}</span>
              <span style={{ flex: 1, paddingLeft: 14, whiteSpace: "pre", color: "var(--fg)" }}>
                <CodeLine text={row.code} />
              </span>
              {row.panic && (
                <span style={{
                  position: "absolute", right: 18, top: 2,
                  fontSize: 10, color: "var(--bad)", letterSpacing: "0.1em",
                  background: "var(--bg)", padding: "2px 6px", border: "1px solid var(--bad)",
                }}>! ATTEMPT TO ADD WITH OVERFLOW</span>
              )}
            </div>
          ))}
        </div>
        <div style={{
          borderTop: "1px solid var(--rule)", padding: "8px 16px",
          display: "flex", gap: 6, alignItems: "center",
        }}>
          {[
            { l: "▶", n: "CONTINUE", k: "F5" },
            { l: "↷", n: "STEP OVER", k: "F10" },
            { l: "↘", n: "STEP IN", k: "F11" },
            { l: "↖", n: "STEP OUT", k: "⇧F11" },
            { l: "⏹", n: "STOP", k: "⇧F5" },
          ].map((b, i) => (
            <button key={i} style={{
              fontFamily: "var(--mono)", fontSize: 10, letterSpacing: "0.14em",
              padding: "4px 8px", background: "transparent", color: "var(--fg-2)",
              border: "1px solid var(--rule)", cursor: "pointer",
              display: "flex", alignItems: "center", gap: 6,
            }}>
              <span style={{ color: "var(--accent)" }}>{b.l}</span>
              <span>{b.n}</span>
              <span style={{ color: "var(--fg-4)" }}>{b.k}</span>
            </button>
          ))}
        </div>
      </div>

      {/* vars + watches */}
      <aside style={{ borderLeft: "1px solid var(--rule)", display: "flex", flexDirection: "column" }}>
        <PaneHeader fig="06C" title="Variables" right={`${vars.length} in scope`} />
        <div style={{ flex: 1, overflow: "auto", padding: "8px 0" }}>
          {vars.map((v, i) => {
            const danger = v.val.includes("usize::MAX");
            return (
              <div key={i} style={{
                padding: "8px 14px", borderBottom: "1px solid var(--rule-2)",
                background: danger ? "var(--rm)" : "transparent",
              }}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 2 }}>
                  <span style={{ fontSize: 11, color: "var(--fg)" }}>{v.name}</span>
                  <span style={{ fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.1em" }}>{v.ty}</span>
                </div>
                <div style={{ fontSize: 11, color: danger ? "var(--bad)" : "var(--accent)" }}>= {v.val}</div>
              </div>
            );
          })}
        </div>
        <div style={{ borderTop: "1px solid var(--rule)", padding: 12 }}>
          <div className="label-strong" style={{ marginBottom: 6 }}>WATCH</div>
          <div style={{ fontSize: 10, color: "var(--fg-3)", fontStyle: "italic" }}>
            self.inflight.load(Relaxed) → <span style={{ color: "var(--bad)" }}>18446744073709551615</span>
          </div>
        </div>
      </aside>
    </div>
  );
}

/* ═════════════════════════════ SETTINGS ═════════════════════════════ */
function SettingsScreen({ features, setFeatures, theme, setTheme }) {
  const cats = ["GENERAL","APPEARANCE","EDITOR","NOVEL","KEY MAP","EXTENSIONS","ACCOUNT"];
  const [cat, setCat] = useS(3);
  return (
    <div style={{
      flex: 1, display: "grid", gridTemplateColumns: "240px 1fr",
      background: "var(--bg)", minHeight: 0, overflow: "hidden",
    }}>
      <aside style={{ borderRight: "1px solid var(--rule)", display: "flex", flexDirection: "column" }}>
        <PaneHeader fig="09" title="Preferences" right="EDEN v0.1.0" />
        <div style={{ flex: 1, padding: "10px 0" }}>
          {cats.map((c, i) => (
            <div key={i} onClick={() => setCat(i)} style={{
              padding: "8px 16px", fontSize: 11, letterSpacing: "0.16em",
              color: cat === i ? "var(--fg)" : "var(--fg-3)",
              background: cat === i ? "var(--bg-2)" : "transparent",
              borderLeft: cat === i ? "2px solid var(--accent)" : "2px solid transparent",
              cursor: "pointer", display: "flex", justifyContent: "space-between",
            }}>
              <span>{c}</span>
              <span style={{ color: "var(--fg-4)", fontSize: 9 }}>{String(i+1).padStart(2,"0")}</span>
            </div>
          ))}
        </div>
      </aside>
      <div style={{ overflow: "auto", padding: "32px 48px" }}>
        <div className="label" style={{ marginBottom: 6 }}>SECTION {String(cat+1).padStart(2,"0")} OF 07</div>
        <h2 style={{
          fontFamily: "var(--font-display)", fontWeight: 600, fontSize: 32,
          margin: 0, letterSpacing: "var(--display-tracking)",
        }}>{cats[cat]}</h2>
        <p style={{
          fontFamily: "var(--serif)", fontStyle: "italic", fontSize: 14,
          color: "var(--fg-2)", maxWidth: 540, marginTop: 8,
        }}>
          {cat === 3
            ? "EDEN is built around five experimental mechanics. Switch them off if any one breaks your concentration."
            : "Settings for this section."}
        </p>
        <div style={{ borderTop: "1px solid var(--rule)", marginTop: 20, marginBottom: 22 }} />

        {cat === 3 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
            <FeatureRow
              num="01" name="Time Scrubber" k="time"
              body="A horizontal timeline at the top of the editor. Drag it to rewind through every keystroke, commit, and AI patch. Press ⌘⇧T to anchor on a commit."
              on={features.time} set={v => setFeatures({...features, time: v})} />
            <FeatureRow
              num="02" name="Spatial Canvas" k="canvas"
              body="Replace the tree with a zoomable 2D map of every file and its imports. Files cluster by module; edges show dependency direction."
              on={features.canvas} set={v => setFeatures({...features, canvas: v})} />
            <FeatureRow
              num="03" name="Semantic Minimap" k="minimap"
              body="The right rail shows logic flow — scopes, branches, complexity — not a tiny picture of your lines."
              on={features.minimap} set={v => setFeatures({...features, minimap: v})} />
            <FeatureRow
              num="04" name="Tactile Diffs" k="diff"
              body="Changes don't snap into place. They slide, settle, and glow. A small ceremony that says: something changed here."
              on={features.diff} set={v => setFeatures({...features, diff: v})} />
            <FeatureRow
              num="05" name="Depth-of-Field Focus" k="focus"
              body="In focus mode, code outside the active scope is softly defocused so your eye doesn't wander."
              on={features.focus} set={v => setFeatures({...features, focus: v})} />
            <FeatureRow
              num="06" name="Ambient AI" k="ai"
              body="EDEN lives in the margin. Suggestions appear as small editorial notes pinned to the line they care about — never as a chat panel."
              on={features.ai} set={v => setFeatures({...features, ai: v})} />
            <FeatureRow
              num="07" name="Code Breath" k="breath"
              body="A subtle, ambient grid drift in the background. Off by default — turn it on if you want the editor to feel alive."
              on={features.breath} set={v => setFeatures({...features, breath: v})} />
          </div>
        )}

        {cat !== 3 && (
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
            {[
              { k: "Theme",       v: theme.toUpperCase() },
              { k: "Font face",   v: "JetBrains Mono · 12.5px" },
              { k: "Tab width",   v: "4 spaces" },
              { k: "Format on save", v: "rustfmt · enabled" },
              { k: "Auto save",   v: "after 600ms idle" },
              { k: "Vim mode",    v: "off" },
            ].map((r, i) => (
              <div key={i} style={{
                display: "flex", flexDirection: "column", gap: 4,
                padding: "12px 0", borderBottom: "1px solid var(--rule)",
              }}>
                <span className="label">{r.k}</span>
                <span style={{ fontSize: 14, color: "var(--fg)" }}>{r.v}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function FeatureRow({ num, name, body, on, set }) {
  return (
    <div style={{
      display: "grid", gridTemplateColumns: "44px 1fr 90px",
      gap: 24, padding: "20px 0", borderBottom: "1px solid var(--rule)",
      alignItems: "flex-start",
    }}>
      <span className="label num" style={{ paddingTop: 2 }}>{num}</span>
      <div>
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <span style={{ fontSize: 16, color: "var(--fg)", fontWeight: 500 }}>{name}</span>
          <span className="label" style={{
            color: on ? "var(--accent)" : "var(--fg-4)",
          }}>{on ? "● ON" : "○ OFF"}</span>
        </div>
        <p style={{
          margin: "6px 0 0", fontSize: 12, color: "var(--fg-2)",
          maxWidth: 540, fontFamily: "var(--serif)", fontStyle: "italic",
        }}>{body}</p>
      </div>
      <button onClick={() => set(!on)} style={{
        fontFamily: "var(--mono)", fontSize: 10, letterSpacing: "0.16em",
        padding: "6px 0", width: 80,
        background: on ? "var(--accent)" : "transparent",
        color: on ? "var(--bg)" : "var(--fg)",
        border: on ? "1px solid var(--accent)" : "1px solid var(--fg)",
        cursor: "pointer",
        transition: "all 140ms var(--ease)",
      }}>{on ? "ENABLED" : "ENABLE"}</button>
    </div>
  );
}

/* ═════════════════════════════ AI PAIR ═════════════════════════════ */
function AIPairScreen() {
  return (
    <div style={{ flex: 1, display: "grid", gridTemplateColumns: "1fr 1.4fr", background: "var(--bg)", minHeight: 0, overflow: "hidden" }}>
      {/* left manifesto */}
      <div style={{
        padding: "56px 48px", borderRight: "1px solid var(--rule)",
        display: "flex", flexDirection: "column", justifyContent: "space-between",
      }}>
        <div>
          <span className="label">FIG. 08 — AMBIENT PAIR</span>
          <h2 style={{
            fontFamily: "var(--font-display)", fontSize: 56, lineHeight: 0.95,
            margin: "12px 0 0", letterSpacing: "var(--display-tracking)",
          }}>The pair<br/>that doesn't<br/><span style={{ fontFamily: "var(--serif)", fontStyle: "italic", color: "var(--accent)" }}>interrupt.</span></h2>
          <p style={{ fontSize: 13, color: "var(--fg-2)", maxWidth: 380, marginTop: 18, lineHeight: 1.55 }}>
            EDEN lives in the margin. No chat panel, no sidebar that steals
            half your screen. Suggestions appear as small editorial notes,
            pinned to the line they care about. Accept with <b>↵</b>. Dismiss with <b>⎋</b>.
            Ask a longer question with <b>⌥ K</b>.
          </p>
          <div style={{
            marginTop: 26, display: "grid", gap: 6,
            fontSize: 11, color: "var(--fg-3)",
          }}>
            <KV k="Model" v="local (8B) · cloud fallback opt-in" />
            <KV k="Context" v="48k · cargo workspace aware" />
            <KV k="Latency p50" v="64 ms" />
            <KV k="Privacy" v="no telemetry · no training on your code" />
          </div>
        </div>
        <Crosshair />
      </div>

      {/* right convo */}
      <div style={{ display: "flex", flexDirection: "column", padding: "40px 48px", overflow: "auto", gap: 22 }}>
        <Margin role="you" t="01" body={
          <>why does <code style={{color:"var(--accent)"}}>acquire()</code> sometimes hold its permit longer than the target?</>
        } />
        <Margin role="eden" t="02" body={<>
          The <code>Permit</code> in your code only records <code>elapsed</code> on <code>drop</code>. If a downstream <code>.await</code> never returns,
          the permit stays in flight and <code>record</code> is never called — so the aperture can't widen back. Two options:
          <ul style={{margin:"6px 0 0 16px",padding:0}}>
            <li>add a <code style={{color:"var(--accent)"}}>tokio::time::timeout</code> on the permit's lifetime</li>
            <li>or switch to a <i>tick</i> sampler that runs every 100ms regardless of permit lifecycle</li>
          </ul>
        </>} />
        <Margin role="you" t="03" body={
          <>show me the second option</>
        } />
        <Margin role="eden" t="04" code body={
          <>
            <span style={{color:"var(--fg-3)"}}>// in Aperture::new, spawn a sampler</span><br/>
            <span style={{color:"var(--accent)",fontWeight:600}}>tokio::spawn</span>(<span style={{color:"var(--accent)",fontWeight:600}}>async move</span> {"{"}<br/>
            {"  "}<span style={{color:"var(--accent)",fontWeight:600}}>let</span> mut tick = tokio::time::interval(<span>Duration::from_millis</span>(<span style={{color:"var(--warn)"}}>100</span>));<br/>
            {"  "}<span style={{color:"var(--accent)",fontWeight:600}}>loop</span> {"{"}<br/>
            {"    "}tick.tick().<span style={{color:"var(--accent)",fontWeight:600}}>await</span>;<br/>
            {"    "}weak.upgrade().map(|a| a.sample());<br/>
            {"  "}{"}"}<br/>
            {"}"});
          </>
        } />
        <Margin role="eden" t="05" pending body={
          <>preparing patch · 3 files · 18 lines<br/>
          <span style={{color:"var(--fg-3)",fontSize:10,letterSpacing:"0.1em"}}>↵ APPLY  ⎋ DISMISS  ⌘D DIFF</span></>
        } />
      </div>
    </div>
  );
}

function Margin({ role, t, body, code, pending }) {
  const isEden = role === "eden";
  return (
    <div style={{
      display: "grid", gridTemplateColumns: "60px 1fr",
      gap: 14, alignItems: "flex-start",
    }}>
      <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: 4 }}>
        <span className="label" style={{ color: isEden ? "var(--accent)" : "var(--fg-3)" }}>
          {isEden ? "✱ EDEN" : "▲ YOU"}
        </span>
        <span className="label num" style={{ fontSize: 9 }}>{t}</span>
      </div>
      <div style={{
        border: isEden ? "1px solid var(--accent)" : "1px solid var(--rule)",
        padding: "10px 14px",
        background: code ? "var(--bg-elev)" : "transparent",
        fontFamily: code ? "var(--mono)" : "var(--mono)",
        fontSize: code ? 11 : 13,
        lineHeight: code ? 1.6 : 1.5,
        color: "var(--fg)",
        position: "relative",
      }}>
        {pending && (
          <span style={{
            position: "absolute", top: 8, right: 12,
            fontSize: 9, color: "var(--accent)", letterSpacing: "0.16em",
            animation: "pulse 1.6s ease-in-out infinite",
          }}>● COMPOSING</span>
        )}
        {body}
      </div>
    </div>
  );
}

function KV({ k, v }) {
  return (
    <div style={{
      display: "grid", gridTemplateColumns: "120px 1fr",
      gap: 8, borderBottom: "1px solid var(--rule-2)", padding: "5px 0",
    }}>
      <span className="label">{k}</span>
      <span style={{ color: "var(--fg-2)" }}>{v}</span>
    </div>
  );
}

/* ═════════════════════════════ ONBOARDING ═════════════════════════════ */
function OnboardingScreen({ onDone }) {
  const [step, setStep] = useS(0);
  const steps = [
    { n: "01", h: "Welcome to EDEN", s: "An editor that treats time as a dimension and your code as a place.", glyph: <Geom1/> },
    { n: "02", h: "Open a Rust workspace", s: "Point us at any cargo project. We'll index in the background — usually under a minute.", glyph: <Geom2/> },
    { n: "03", h: "Meet the Pair", s: "EDEN's AI lives in the margin. Press ⌥K to ask. Press ⎋ to make it leave.", glyph: <Geom3/> },
    { n: "04", h: "Scrub through time", s: "Every keystroke is a frame. Drag the timeline to rewind. Tag any moment as a 'checkpoint'.", glyph: <Geom4/> },
    { n: "05", h: "Focus when it counts", s: "⌘. enters focus mode. Code outside your scope softens. You and the function — alone.", glyph: <Geom5/> },
  ];
  const cur = steps[step];

  return (
    <div style={{
      flex: 1, display: "grid", gridTemplateRows: "auto 1fr auto",
      background: "var(--bg)", minHeight: 0, overflow: "hidden",
    }}>
      {/* progress */}
      <div style={{
        display: "flex", gap: 0, padding: "20px 48px 0",
        alignItems: "center",
      }}>
        {steps.map((s, i) => (
          <React.Fragment key={i}>
            {i > 0 && <div style={{ flex: 1, height: 1, background: i <= step ? "var(--accent)" : "var(--rule)", margin: "0 4px" }} />}
            <button onClick={() => setStep(i)} style={{
              width: 26, height: 26, border: `1px solid ${i <= step ? "var(--accent)" : "var(--rule)"}`,
              background: i === step ? "var(--accent)" : "transparent",
              color: i === step ? "var(--bg)" : (i < step ? "var(--accent)" : "var(--fg-3)"),
              fontFamily: "var(--mono)", fontSize: 10, cursor: "pointer", letterSpacing: "0.1em",
            }}>{s.n}</button>
          </React.Fragment>
        ))}
      </div>

      {/* body */}
      <div style={{
        display: "grid", gridTemplateColumns: "1fr 1fr",
        padding: "20px 48px", gap: 64, alignItems: "center",
      }}>
        <div key={step} style={{ animation: "fadeUp 360ms var(--ease)" }}>
          <span className="label">STEP {cur.n} OF 05</span>
          <h1 style={{
            fontFamily: "var(--font-display)", fontSize: 52, lineHeight: 1.0,
            margin: "10px 0 16px", letterSpacing: "var(--display-tracking)",
          }}>{cur.h}</h1>
          <p style={{
            fontFamily: "var(--serif)", fontStyle: "italic",
            fontSize: 20, color: "var(--fg-2)", lineHeight: 1.4, maxWidth: 480, margin: 0,
          }}>{cur.s}</p>
        </div>
        <div style={{
          display: "flex", alignItems: "center", justifyContent: "center",
          border: "1px solid var(--rule)",
          aspectRatio: "1 / 1", maxHeight: 360, position: "relative",
        }}>
          {cur.glyph}
          <div style={{
            position: "absolute", top: 10, left: 12,
            fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.18em",
          }}>FIG. 05.{cur.n}</div>
          <div style={{
            position: "absolute", bottom: 10, right: 12,
            fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.18em",
          }}>0:0:{cur.n}</div>
        </div>
      </div>

      {/* footer */}
      <div style={{
        borderTop: "1px solid var(--rule)", padding: "16px 48px",
        display: "flex", alignItems: "center", justifyContent: "space-between",
      }}>
        <span style={{ fontSize: 11, color: "var(--fg-3)" }}>
          PRESS <kbd style={kbd}>→</kbd> NEXT · <kbd style={kbd}>←</kbd> BACK · <kbd style={kbd}>ESC</kbd> SKIP
        </span>
        <div style={{ display: "flex", gap: 8 }}>
          <BigBtn onClick={() => onDone()}>SKIP</BigBtn>
          {step > 0 && <BigBtn onClick={() => setStep(step-1)}>← BACK</BigBtn>}
          {step < steps.length-1
            ? <BigBtn primary onClick={() => setStep(step+1)}>NEXT →</BigBtn>
            : <BigBtn primary onClick={() => onDone()}>ENTER EDEN ↗</BigBtn>}
        </div>
      </div>
    </div>
  );
}

const kbd = {
  display: "inline-block", padding: "1px 6px", margin: "0 2px",
  border: "1px solid var(--rule)", fontSize: 10, color: "var(--fg-2)",
  letterSpacing: "0.1em",
};

/* simple geometric glyphs for onboarding */
function Geom1() { // welcome — diamond
  return (
    <svg viewBox="0 0 200 200" width="68%">
      <path d="M100 20 L180 100 L100 180 L20 100 Z" stroke="var(--fg)" strokeWidth="1" fill="none" />
      <path d="M100 60 L140 100 L100 140 L60 100 Z" stroke="var(--accent)" strokeWidth="1.5" fill="none" />
      <circle cx="100" cy="100" r="4" fill="var(--accent)" />
    </svg>
  );
}
function Geom2() { // workspace — nested squares
  return (
    <svg viewBox="0 0 200 200" width="68%">
      {[80,60,40,20].map((s,i) => (
        <rect key={i} x={100-s} y={100-s} width={s*2} height={s*2}
          stroke={i===0?"var(--accent)":"var(--fg-3)"} fill="none"
          strokeDasharray={i===3?"3 3":"none"} />
      ))}
      <text x="100" y="105" textAnchor="middle" fontFamily="var(--mono)" fontSize="10" fill="var(--fg-3)" letterSpacing="0.2em">CRATE</text>
    </svg>
  );
}
function Geom3() { // pair — speech bracket
  return (
    <svg viewBox="0 0 200 200" width="68%">
      <path d="M40 50 L160 50 L160 130 L100 130 L80 160 L80 130 L40 130 Z" stroke="var(--fg)" fill="none" />
      <text x="100" y="95" textAnchor="middle" fontFamily="var(--mono)" fontSize="32" fill="var(--accent)">✱</text>
    </svg>
  );
}
function Geom4() { // time — ticks
  return (
    <svg viewBox="0 0 200 200" width="80%">
      <line x1="10" y1="100" x2="190" y2="100" stroke="var(--fg-3)" />
      {Array.from({length:13},(_,i)=> (
        <line key={i} x1={10+i*15} y1={i%4===0?92:96} x2={10+i*15} y2={i%4===0?108:104} stroke="var(--fg-3)" />
      ))}
      <circle cx="115" cy="100" r="6" fill="var(--accent)" />
      <line x1="115" y1="80" x2="115" y2="120" stroke="var(--accent)" />
      <text x="115" y="70" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="var(--accent)" letterSpacing="0.2em">NOW</text>
      <text x="40" y="135" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="var(--fg-4)" letterSpacing="0.1em">T-30m</text>
      <text x="180" y="135" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="var(--fg-4)" letterSpacing="0.1em">T-0</text>
    </svg>
  );
}
function Geom5() { // focus — concentric blur
  return (
    <svg viewBox="0 0 200 200" width="68%">
      <defs>
        <filter id="bl"><feGaussianBlur stdDeviation="2.4" /></filter>
      </defs>
      <g filter="url(#bl)" opacity="0.5">
        <rect x="20" y="40" width="160" height="6" fill="var(--fg-3)" />
        <rect x="20" y="56" width="120" height="6" fill="var(--fg-3)" />
        <rect x="20" y="72" width="140" height="6" fill="var(--fg-3)" />
      </g>
      <rect x="20" y="94" width="160" height="8" fill="var(--accent)" />
      <rect x="20" y="106" width="100" height="8" fill="var(--fg)" />
      <rect x="20" y="118" width="140" height="8" fill="var(--fg)" />
      <g filter="url(#bl)" opacity="0.4">
        <rect x="20" y="138" width="120" height="6" fill="var(--fg-3)" />
        <rect x="20" y="154" width="160" height="6" fill="var(--fg-3)" />
      </g>
    </svg>
  );
}

/* ═════════════════════════════ SPATIAL CANVAS ═════════════════════════════ */
function SpatialCanvas() {
  // file clusters
  const clusters = [
    { id: "aperture", x: 360, y: 240, r: 130, color: "var(--accent)",
      files: [
        { name: "lib.rs",     x: 0,   y: 0,   active: true, size: 14 },
        { name: "permit.rs",  x: -60, y: -42, size: 10 },
        { name: "metrics.rs", x: 70,  y: -30, size: 11 },
        { name: "tests.rs",   x: 0,   y: 70,  size: 9 },
        { name: "errors.rs",  x: -70, y: 40,  size: 8 },
      ]},
    { id: "ingest",   x: 730, y: 180, r: 92,
      files: [
        { name: "mod.rs",     x: 0, y: 0, size: 12 },
        { name: "queue.rs",   x: -52, y: 30, size: 10 },
        { name: "decode.rs",  x: 50, y: -20, size: 10 },
      ]},
    { id: "store",    x: 740, y: 430, r: 100,
      files: [
        { name: "mod.rs",     x: 0, y: 0, size: 12 },
        { name: "wal.rs",     x: -56, y: -36, size: 11 },
        { name: "index.rs",   x: 60, y: -10, size: 11 },
        { name: "compact.rs", x: 0, y: 56, size: 10 },
      ]},
    { id: "runtime",  x: 130, y: 480, r: 86,
      files: [
        { name: "main.rs",    x: 0, y: 0, size: 13 },
        { name: "boot.rs",    x: 40, y: -40, size: 9 },
        { name: "shutdown.rs",x: -40, y: 30, size: 9 },
      ]},
  ];
  const edges = [
    ["runtime","aperture"],
    ["aperture","ingest"],
    ["aperture","store"],
    ["ingest","store"],
  ];
  const cMap = Object.fromEntries(clusters.map(c => [c.id, c]));

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", background: "var(--bg)", minHeight: 0 }}>
      <PaneHeader fig="02" title="Spatial Canvas · hyperion" right="ZOOM 1.0× · ⌥ DRAG PAN · ⌘ +/- ZOOM" />
      <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
        {/* dot grid */}
        <div style={{
          position: "absolute", inset: 0,
          backgroundImage: "radial-gradient(var(--rule) 1px, transparent 1px)",
          backgroundSize: "22px 22px", opacity: 0.5,
        }} />
        {/* coord crosshair */}
        <div style={{
          position: "absolute", top: 10, left: 10,
          fontSize: 9, color: "var(--fg-4)", letterSpacing: "0.18em",
        }}>X: 0  Y: 0  Z: 1.00</div>

        <svg style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}>
          {/* edges */}
          {edges.map(([a,b], i) => {
            const A = cMap[a], B = cMap[b];
            return (
              <g key={i}>
                <line x1={A.x} y1={A.y} x2={B.x} y2={B.y}
                  stroke="var(--fg-4)" strokeWidth="1" strokeDasharray="3 4" />
                {/* arrow head */}
                <ArrowHead x1={A.x} y1={A.y} x2={B.x} y2={B.y} />
              </g>
            );
          })}
        </svg>

        {clusters.map((c, i) => (
          <Cluster key={c.id} c={c} />
        ))}

        {/* legend */}
        <div style={{
          position: "absolute", bottom: 14, left: 14,
          background: "var(--bg-elev)", border: "1px solid var(--rule)",
          padding: 12, fontSize: 10, display: "flex", flexDirection: "column", gap: 6,
          minWidth: 160,
        }}>
          <div className="label-strong">FIG. 02 — LEGEND</div>
          <Legend dot="var(--accent)" label="active crate" />
          <Legend dot="var(--fg)" label="hot file (∂ today)" />
          <Legend dot="var(--fg-3)" label="cold file" />
          <Legend line label="import edge" />
        </div>

        {/* meta */}
        <div style={{
          position: "absolute", top: 14, right: 14, textAlign: "right",
          fontSize: 10, color: "var(--fg-3)", letterSpacing: "0.1em",
        }}>
          <div>4 CRATES · 47 FILES · 12 EDGES</div>
          <div style={{ color: "var(--fg-4)" }}>RENDERED @ 60 FPS</div>
        </div>
      </div>
    </div>
  );
}

function ArrowHead({ x1, y1, x2, y2 }) {
  const ang = Math.atan2(y2-y1, x2-x1);
  const cx = (x1+x2)/2, cy = (y1+y2)/2;
  const s = 6;
  return (
    <polygon
      points={`${cx},${cy} ${cx - s*Math.cos(ang - 0.4)},${cy - s*Math.sin(ang - 0.4)} ${cx - s*Math.cos(ang + 0.4)},${cy - s*Math.sin(ang + 0.4)}`}
      fill="var(--fg-4)"
    />
  );
}

function Cluster({ c }) {
  return (
    <div style={{
      position: "absolute", left: c.x - c.r, top: c.y - c.r,
      width: c.r*2, height: c.r*2,
      border: `1px solid ${c.color || "var(--rule)"}`,
      borderRadius: 0,
    }}>
      {/* label */}
      <div style={{
        position: "absolute", top: -22, left: 0,
        fontSize: 10, letterSpacing: "0.18em",
        color: c.color || "var(--fg-3)", textTransform: "uppercase",
      }}>{c.id}</div>

      {c.files.map((f, i) => (
        <div key={i} style={{
          position: "absolute",
          left: c.r + f.x, top: c.r + f.y,
          transform: "translate(-50%, -50%)",
          display: "flex", flexDirection: "column", alignItems: "center", gap: 4,
        }}>
          <div style={{
            width: f.size*2, height: f.size*2,
            border: `1px solid ${f.active ? "var(--accent)" : "var(--fg-3)"}`,
            background: f.active ? "var(--accent)" : "var(--bg)",
          }} />
          <span style={{
            fontSize: 9, color: f.active ? "var(--fg)" : "var(--fg-3)",
            whiteSpace: "nowrap",
          }}>{f.name}</span>
        </div>
      ))}
    </div>
  );
}

function Legend({ dot, line, label }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--fg-2)" }}>
      {dot && <div style={{ width: 8, height: 8, background: dot }} />}
      {line && <div style={{ width: 14, borderTop: "1px dashed var(--fg-4)" }} />}
      <span>{label}</span>
    </div>
  );
}

/* ═════════════════════════════ shared ═════════════════════════════ */
function PaneHeader({ fig, title, right, rightAccent }) {
  return (
    <div style={{
      padding: "12px 16px 10px", display: "flex", alignItems: "baseline",
      justifyContent: "space-between", borderBottom: "1px solid var(--rule)",
      flexShrink: 0,
    }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
        <span className="label-strong">FIG. {fig}</span>
        <span style={{ fontSize: 14, color: "var(--fg)", letterSpacing: "-0.01em" }}>{title}</span>
      </div>
      <span className="label" style={{ color: rightAccent ? "var(--accent)" : "var(--fg-3)" }}>{right}</span>
    </div>
  );
}

Object.assign(window, {
  WelcomeScreen, CommandPalette, TerminalScreen, DebuggerScreen,
  SettingsScreen, AIPairScreen, OnboardingScreen, SpatialCanvas, PaneHeader, BigBtn,
});
