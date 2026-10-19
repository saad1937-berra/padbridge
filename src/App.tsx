// src/App.tsx — navigation principale et état partagé.

import { useEffect, useState } from "react";
import { useControllers, useProfiles, useSettings } from "./lib/useEngine";
import { Home, type Screen } from "./screens/Home";
import { Devices } from "./screens/Devices";
import { TestScreen } from "./screens/TestScreen";
import { Profiles } from "./screens/Profiles";
import { Compatibility } from "./screens/Compatibility";
import { Diagnostics } from "./screens/Diagnostics";
import { Settings } from "./screens/Settings";

const NAV: { id: Screen; label: string; icon: string }[] = [
  { id: "home", label: "Accueil", icon: "⌂" },
  { id: "devices", label: "Appareils", icon: "◫" },
  { id: "test", label: "Test manette", icon: "✚" },
  { id: "profiles", label: "Profils", icon: "☰" },
  { id: "compat", label: "Compatibilité", icon: "⇄" },
  { id: "diag", label: "Diagnostics", icon: "⚕" },
  { id: "settings", label: "Paramètres", icon: "⚙" },
];

export default function App() {
  const [screen, setScreen] = useState<Screen>("home");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { controllers, loading, refresh } = useControllers();
  const { profiles, refresh: refreshProfiles } = useProfiles();
  const { settings, update } = useSettings();

  useEffect(() => {
    const root = document.documentElement;
    if (settings && settings.theme !== "system") root.dataset.theme = settings.theme;
    else delete root.dataset.theme;
  }, [settings?.theme]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="flex h-full">
      <nav className="flex w-56 shrink-0 flex-col gap-1 border-r p-3" style={{ borderColor: "var(--border)", background: "var(--surface)" }} aria-label="Navigation principale">
        <div className="px-3 pb-3 pt-1 text-lg font-bold">PadBridge</div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className="nav-item"
            aria-current={screen === item.id ? "page" : undefined}
            onClick={() => setScreen(item.id)}
          >
            <span aria-hidden="true" className="w-5 text-center">{item.icon}</span>
            {item.label}
          </button>
        ))}
        <p className="muted mt-auto px-3 text-xs">
          Projet indépendant, non affilié à Sony ni à Microsoft.
        </p>
      </nav>

      <main className="min-w-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-5xl">
          {screen === "home" && (
            <Home
              controllers={controllers} loading={loading} refresh={refresh} profiles={profiles}
              settings={settings} updateSettings={update} selectDevice={setSelectedId} goto={setScreen}
            />
          )}
          {screen === "devices" && (
            <Devices controllers={controllers} refresh={refresh} selectedId={selectedId} selectDevice={setSelectedId} />
          )}
          {screen === "test" && (
            <TestScreen controllers={controllers} selectedId={selectedId} selectDevice={setSelectedId} settings={settings} />
          )}
          {screen === "profiles" && (
            <Profiles
              profiles={profiles} refreshProfiles={refreshProfiles} controllers={controllers}
              settings={settings} updateSettings={update} refreshControllers={refresh}
            />
          )}
          {screen === "compat" && <Compatibility controllers={controllers} goto={setScreen} />}
          {screen === "diag" && <Diagnostics />}
          {screen === "settings" && <Settings settings={settings} update={update} />}
        </div>
      </main>
    </div>
  );
}
