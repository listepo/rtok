// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useCallback, useEffect, useState } from "react";

const KEY = "rtok-theme";
const LIGHT_QUERY = "(prefers-color-scheme: light)";

function stored(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    // Storage can be blocked (private mode, file://); the theme then follows the OS.
    return null;
  }
}

// Same rule as the pre-paint script in index.html, which cannot import this module.
export function resolveDark(): boolean {
  const saved = stored();
  return saved ? saved === "dark" : !matchMedia(LIGHT_QUERY).matches;
}

function apply(dark: boolean) {
  const root = document.documentElement;
  root.classList.toggle("dark", dark);
  root.classList.toggle("light", !dark);
  root.setAttribute("data-theme", dark ? "dark" : "light");
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", dark ? "#06101A" : "#F4F8FB");
}

export function useTheme() {
  const [dark, setDark] = useState(resolveDark);

  useEffect(() => apply(dark), [dark]);

  useEffect(() => {
    const query = matchMedia(LIGHT_QUERY);
    // An explicit choice wins over the OS; only an unset theme tracks it.
    const onChange = () => {
      if (!stored()) setDark(resolveDark());
    };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const toggle = useCallback(() => {
    const next = !dark;
    try {
      localStorage.setItem(KEY, next ? "dark" : "light");
    } catch {
      // Not persisted; the choice still applies for this session.
    }
    setDark(next);
  }, [dark]);

  return { dark, toggle };
}
