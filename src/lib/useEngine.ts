// src/lib/useEngine.ts — hooks d'accès au moteur.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, onControllersChanged, onInputSample } from "./api";
import type { AppSettings, ControllerView, InputSample, Profile } from "./types";

export function useControllers() {
  const [controllers, setControllers] = useState<ControllerView[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setControllers(await api.listControllers());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    let off: (() => void) | undefined;
    let cancelled = false;
    void onControllersChanged(() => void refresh()).then((u) => {
      if (cancelled) u();
      else off = u;
    });
    return () => {
      cancelled = true;
      off?.();
    };
  }, [refresh]);

  return { controllers, loading, refresh };
}

/** Abonnement aux échantillons d'une manette ; `preview` = profil brouillon (aperçu en direct). */
export function useInputSample(deviceId: string | null, preview?: Profile | null) {
  const [sample, setSample] = useState<InputSample | null>(null);
  const previewKey = useMemo(() => (preview ? JSON.stringify(preview) : ""), [preview]);
  const previewRef = useRef(preview);
  previewRef.current = preview;

  useEffect(() => {
    setSample(null);
    if (!deviceId) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    void onInputSample((s) => {
      if (s.deviceId === deviceId) setSample(s);
    }).then((u) => {
      if (cancelled) u();
      else off = u;
    });
    return () => {
      cancelled = true;
      off?.();
      void api.stopInputTest();
    };
  }, [deviceId]);

  useEffect(() => {
    if (!deviceId) return;
    const timer = setTimeout(() => {
      void api.startInputTest(deviceId, previewRef.current ?? null).catch(() => undefined);
    }, 80); // regroupe les déplacements rapides de curseurs
    return () => clearTimeout(timer);
  }, [deviceId, previewKey]);

  return sample;
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);

  useEffect(() => {
    void api.getSettings().then(setSettings);
  }, []);

  const update = useCallback(async (next: AppSettings) => {
    const saved = await api.saveSettings(next);
    setSettings(saved);
    return saved;
  }, []);

  return { settings, update };
}

export function useProfiles() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const refresh = useCallback(async () => setProfiles(await api.listProfiles()), []);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  return { profiles, refresh };
}
