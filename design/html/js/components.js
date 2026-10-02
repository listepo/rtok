// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

// rtok design — components page demos (toast, dialog, switch, chip). No deps.
(function () {
  "use strict";
  document.addEventListener("click", function (e) {
    var t = e.target.closest(
      "[data-demo-toast],[data-demo-modal],[data-demo-switch],[data-demo-chip]",
    );
    if (!t || t.disabled || t.tabIndex === -1) return;
    if (t.hasAttribute("data-demo-toast")) {
      var d = document.createElement("div");
      d.className =
        "glass px-3 py-2 text-xs shadow-e3 max-w-xs" +
        (t.dataset.demoToast === "warn" ? " border-warn/60" : "");
      d.textContent =
        t.dataset.demoToast === "warn"
          ? "upstream 529 overloaded, retrying in 2s"
          : "sample data: graph disabled locally (nothing sent)";
      document.getElementById("toasts").appendChild(d);
      setTimeout(function () {
        d.remove();
      }, 3200);
    } else if (t.hasAttribute("data-demo-modal")) {
      document.getElementById("demo-modal").showModal();
    } else if (t.hasAttribute("data-demo-switch")) {
      t.setAttribute("aria-checked", String(t.getAttribute("aria-checked") !== "true"));
    } else if (t.hasAttribute("data-demo-chip")) {
      t.setAttribute("aria-pressed", String(t.getAttribute("aria-pressed") !== "true"));
    }
  });
})();
