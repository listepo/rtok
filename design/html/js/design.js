// rtok design — shared by every page: light/dark theme toggle.
// The pre-paint snippet in each <head> applies the saved choice (localStorage 'rtok-theme',
// the key the Slint UI and the web admin use) or prefers-color-scheme when none is saved.
(function () {
  "use strict";
  var root = document.documentElement;
  var mq = window.matchMedia("(prefers-color-scheme: light)");
  function saved() {
    try {
      return localStorage.getItem("rtok-theme");
    } catch {
      return null;
    }
  }
  function apply(dark) {
    root.classList.toggle("dark", dark);
    root.classList.toggle("light", !dark);
    root.setAttribute("data-theme", dark ? "dark" : "light");
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", dark ? "#06101A" : "#F4F8FB");
    var labels = document.querySelectorAll("[data-theme-label]");
    for (var i = 0; i < labels.length; i++) labels[i].textContent = dark ? "Light" : "Dark";
    var btns = document.querySelectorAll("[data-theme-toggle]");
    for (var j = 0; j < btns.length; j++) {
      btns[j].setAttribute("aria-label", dark ? "Switch to light theme" : "Switch to dark theme");
      btns[j].setAttribute("aria-pressed", String(!dark));
    }
    document.dispatchEvent(new CustomEvent("rtok:theme", { detail: { dark: dark } }));
  }
  function isDark() {
    return root.classList.contains("dark");
  }
  window.rtokTheme = {
    get dark() {
      return isDark();
    },
    set: function (dark) {
      try {
        localStorage.setItem("rtok-theme", dark ? "dark" : "light");
      } catch {
        /* file:// with storage off */
      }
      apply(dark);
    },
    toggle: function () {
      this.set(!isDark());
    },
  };
  document.addEventListener("click", function (e) {
    var t = e.target.closest && e.target.closest("[data-theme-toggle]");
    if (t) {
      e.preventDefault();
      window.rtokTheme.toggle();
    }
  });
  var onScheme = function () {
    if (!saved()) apply(!mq.matches);
  };
  if (mq.addEventListener) mq.addEventListener("change", onScheme);
  else if (mq.addListener) mq.addListener(onScheme);
  apply(isDark());
})();
