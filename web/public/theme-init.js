// Theme before first paint, same localStorage key the design reference uses ('rtok-theme');
// default follows prefers-color-scheme. A separate file, not an inline script, so the
// Content-Security-Policy `rtok web` sends can forbid inline scripts (T310.9).
(function () {
  var s = null;
  try {
    s = localStorage.getItem("rtok-theme");
  } catch {}
  var dark = s ? s === "dark" : !matchMedia("(prefers-color-scheme: light)").matches;
  var root = document.documentElement;
  root.classList.toggle("dark", dark);
  root.classList.toggle("light", !dark);
  root.setAttribute("data-theme", dark ? "dark" : "light");
  try {
    if (localStorage.getItem("rtok-opaque") === "on") root.classList.add("opaque");
  } catch {}
})();
