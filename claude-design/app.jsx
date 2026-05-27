/* APP.JSX — top-level: routing, theme, density, scrub, tweaks */

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "brutal-dark",
  "feat_time": true,
  "feat_canvas": true,
  "feat_minimap": true,
  "feat_diff": true,
  "feat_focus": true,
  "feat_ai": true,
  "feat_breath": false
}/*EDITMODE-END*/;

const THEMES = [
  { value: "brutal-dark",  label: "01 · Brutal · Dark",   short: "BRUTAL DARK" },
  { value: "brutal-light", label: "02 · Brutal · Light",  short: "BRUTAL LIGHT" },
  { value: "tokyo89",      label: "03 · 東京 89 (Tokyo)",    short: "東京 89" },
  { value: "pacific",      label: "04 · Pacific (Apple)", short: "PACIFIC" },
  { value: "phosphor",     label: "05 · Phosphor (CRT)",  short: "PHOSPHOR" },
  { value: "newsprint",    label: "06 · Broadsheet",       short: "BROADSHEET" },
];

const SCREENS = [
  { id: "welcome",     label: "Welcome",      n: "01" },
  { id: "onboarding",  label: "Onboarding",   n: "02" },
  { id: "editor",      label: "Editor",       n: "03", star: true },
  { id: "spatial",     label: "Spatial",      n: "04" },
  { id: "palette",     label: "Palette",      n: "05" },
  { id: "terminal",    label: "Terminal",     n: "06" },
  { id: "debug",       label: "Debugger",     n: "07" },
  { id: "ai",          label: "AI Pair",      n: "08" },
  { id: "settings",    label: "Settings",     n: "09" },
];

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [screen, setScreen] = React.useState("editor");
  const [density, setDensity] = React.useState("work"); // "work" | "focus"
  const [scrubT, setScrubT] = React.useState(1); // 0..1, 1 = NOW
  const [scrubbing, setScrubbing] = React.useState(false);
  const [paletteOpen, setPaletteOpen] = React.useState(false);

  // sync theme to <html>
  React.useEffect(() => {
    document.documentElement.setAttribute("data-theme", t.theme || "brutal-dark");
  }, [t.theme]);

  // keyboard shortcuts
  React.useEffect(() => {
    const onKey = (e) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key === "p") { e.preventDefault(); setPaletteOpen(true); }
      else if (meta && e.key === ".") { e.preventDefault(); setDensity(d => d === "focus" ? "work" : "focus"); }
      else if (meta && e.shiftKey && e.key === "C") { e.preventDefault(); setScreen("spatial"); }
      else if (e.key === "Escape") { setPaletteOpen(false); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const features = {
    time:    t.feat_time,
    canvas:  t.feat_canvas,
    minimap: t.feat_minimap,
    diff:    t.feat_diff,
    focus:   t.feat_focus,
    ai:      t.feat_ai,
    breath:  t.feat_breath,
  };
  const setFeatures = (next) => {
    const patch = {};
    Object.entries(next).forEach(([k, v]) => { patch["feat_" + k] = v; });
    setTweak(patch);
  };

  const toggleTheme = () => {
    const idx = THEMES.findIndex(x => x.value === t.theme);
    const next = THEMES[(idx + 1) % THEMES.length];
    setTweak("theme", next.value);
  };
  const currentTheme = THEMES.find(x => x.value === t.theme) || THEMES[0];

  // show scrubber only in editor screen
  const showScrubber = screen === "editor" && features.time;

  return (
    <div style={{
      width: "100vw", height: "100vh",
      display: "flex", flexDirection: "column",
      background: "var(--bg)", color: "var(--fg)",
      overflow: "hidden",
    }}>
      <DemoStrip screen={screen} setScreen={setScreen} theme={t.theme} setPaletteOpen={setPaletteOpen} />

      <TopBar
        screen={screen} setScreen={setScreen}
        theme={t.theme} toggleTheme={toggleTheme} themeLabel={currentTheme.short}
        density={density} setDensity={setDensity}
        scrubT={scrubT} setScrubT={setScrubT}
        scrubbing={scrubbing} setScrubbing={setScrubbing}
        showScrubber={showScrubber}
      />

      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <LeftRail screen={screen} setScreen={setScreen} />
        <main style={{ flex: 1, display: "flex", minWidth: 0, minHeight: 0, position: "relative" }}>
          <ScreenRouter
            screen={screen} setScreen={setScreen}
            density={density} features={features} setFeatures={setFeatures}
            scrubT={scrubT} scrubbing={scrubbing}
            theme={t.theme} setTheme={(v) => setTweak("theme", v)}
          />
          {paletteOpen && <CommandPalette onClose={() => setPaletteOpen(false)} />}
        </main>
      </div>

      <BottomHint setPaletteOpen={setPaletteOpen} />

      <TweaksPanel title="TWEAKS · EDEN">
        <TweakSection label="Theme" />
        <TweakSelect
          label="palette"
          value={t.theme}
          onChange={(v) => setTweak("theme", v)}
          options={THEMES.map(x => ({ value: x.value, label: x.label }))}
        />

        <TweakSection label="Novel features" />
        <TweakToggle label="01 · Time scrubber"     value={t.feat_time}    onChange={(v) => setTweak("feat_time", v)} />
        <TweakToggle label="02 · Spatial canvas"    value={t.feat_canvas}  onChange={(v) => setTweak("feat_canvas", v)} />
        <TweakToggle label="03 · Semantic minimap"  value={t.feat_minimap} onChange={(v) => setTweak("feat_minimap", v)} />
        <TweakToggle label="04 · Tactile diffs"     value={t.feat_diff}    onChange={(v) => setTweak("feat_diff", v)} />
        <TweakToggle label="05 · DOF focus mode"    value={t.feat_focus}   onChange={(v) => setTweak("feat_focus", v)} />
        <TweakToggle label="06 · Ambient AI"        value={t.feat_ai}      onChange={(v) => setTweak("feat_ai", v)} />
        <TweakToggle label="07 · Code breath"       value={t.feat_breath}  onChange={(v) => setTweak("feat_breath", v)} />
      </TweaksPanel>
    </div>
  );
}

/* Demo strip — top nav for prototype navigation */
function DemoStrip({ screen, setScreen, theme, setPaletteOpen }) {
  return (
    <div style={{
      display: "flex", alignItems: "stretch",
      borderBottom: "1px solid var(--rule)",
      background: "var(--bg-elev)",
      flexShrink: 0, height: 28,
      fontSize: 10, letterSpacing: "0.14em", textTransform: "uppercase",
    }}>
      <div style={{
        padding: "0 14px", display: "flex", alignItems: "center", gap: 8,
        borderRight: "1px solid var(--rule)", color: "var(--accent)",
      }}>
        <span style={{ fontWeight: 700 }}>EDEN</span>
        <span style={{ color: "var(--fg-4)" }}>· DEMO ROOM</span>
      </div>
      {SCREENS.map(s => {
        const active = s.id === screen;
        return (
          <button key={s.id} onClick={() => setScreen(s.id)} style={{
            padding: "0 12px", display: "flex", alignItems: "center", gap: 6,
            background: active ? "var(--bg)" : "transparent",
            color: active ? "var(--fg)" : "var(--fg-3)",
            border: "none", borderRight: "1px solid var(--rule)",
            borderBottom: active ? "1px solid var(--bg)" : "1px solid var(--rule)",
            marginBottom: active ? -1 : 0,
            fontFamily: "var(--mono)", fontSize: 10,
            letterSpacing: "0.14em", textTransform: "uppercase",
            cursor: "pointer", transition: "color 100ms var(--ease)",
            position: "relative",
          }}>
            {active && <span style={{ position:"absolute", top:0, left:0, right:0, height: 2, background: "var(--accent)" }} />}
            <span style={{ color: "var(--fg-4)" }}>{s.n}</span>
            <span>{s.label}</span>
            {s.star && <span style={{ color: "var(--accent)" }}>★</span>}
          </button>
        );
      })}
      <div style={{ flex: 1 }} />
      <button onClick={() => setPaletteOpen(true)} style={{
        padding: "0 14px", background: "transparent", color: "var(--fg-3)",
        border: "none", borderLeft: "1px solid var(--rule)",
        fontFamily: "var(--mono)", fontSize: 10, letterSpacing: "0.14em",
        textTransform: "uppercase", cursor: "pointer",
      }}>⌘P · Palette</button>
    </div>
  );
}

function BottomHint() {
  return null;
}

/* Screen router */
function ScreenRouter(props) {
  const { screen, setScreen, density, features, setFeatures, scrubT, scrubbing, theme, setTheme } = props;
  switch (screen) {
    case "welcome":
      return <WelcomeScreen onOpen={(target) => setScreen(target === "first" ? "onboarding" : target)} />;
    case "onboarding":
      return <OnboardingScreen onDone={() => setScreen("editor")} />;
    case "editor":
      return <MainEditor density={density} features={features} scrubT={scrubT} scrubbing={scrubbing} />;
    case "spatial":
      return <SpatialCanvas />;
    case "palette": {
      // open as overlay over editor
      return (
        <>
          <MainEditor density={density} features={features} scrubT={scrubT} scrubbing={scrubbing} />
          <CommandPalette onClose={() => setScreen("editor")} />
        </>
      );
    }
    case "terminal":
      return <TerminalScreen />;
    case "debug":
      return <DebuggerScreen />;
    case "ai":
      return <AIPairScreen />;
    case "settings":
      return <SettingsScreen features={features} setFeatures={setFeatures} theme={theme} setTheme={setTheme} />;
    default:
      return <MainEditor density={density} features={features} scrubT={scrubT} scrubbing={scrubbing} />;
  }
}

/* mount */
const root = ReactDOM.createRoot(document.getElementById("root"));
root.render(<App />);
