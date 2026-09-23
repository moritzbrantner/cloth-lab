"use strict";

// Native Pages adapter: keep existing control IDs/events and Rust ownership.
// Install before app.js so incomplete drafts never reach simulation listeners.
(function () {
  const integerIds = new Set(["mesh-resolution", "solver-iterations"]);
  const decimal = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i;

  function parseDraft(raw, min = -Infinity, max = Infinity, integer = false) {
    const text = raw.trim().replace(",", ".");
    if (!decimal.test(text)) return null;
    const value = Number(text);
    return Number.isFinite(value) && value >= min && value <= max &&
      (!integer || Number.isSafeInteger(value)) ? value : null;
  }

  function decimalPlaces(value) {
    const [coefficient, exponent = "0"] = String(value).toLowerCase().split("e");
    return Math.max(0, (coefficient.split(".")[1]?.length ?? 0) - Number(exponent));
  }

  function nudge(value, increment) {
    const precision = Math.max(decimalPlaces(value), decimalPlaces(increment));
    return precision > 100 ? value + increment : Number((value + increment).toFixed(precision));
  }

  function attach(input, integer = false) {
    const declared = input.getAttribute("value");
    const coarseStep = Number(input.step) || 1;
    const fineStep = integer ? 1 : coarseStep / 10;
    const bounds = () => [
      input.min === "" ? -Infinity : Number(input.min),
      input.max === "" ? Infinity : Number(input.max),
    ];
    const read = (raw) => parseDraft(raw, ...bounds(), integer);
    input.type = "text";
    input.inputMode = integer ? "numeric" : "decimal";
    input.setAttribute("role", "spinbutton");
    input.classList.add("exact-number");
    input.title = "Enter an exact value. Enter applies; Escape cancels. Arrow keys adjust; Shift adjusts faster.";
    if (read(declared ?? "") !== null) input.value = declared;
    let committed = input.value;
    const forwarded = new WeakSet();

    function refreshAccessibility() {
      const [min, max] = bounds();
      if (Number.isFinite(min)) input.setAttribute("aria-valuemin", String(min));
      if (Number.isFinite(max)) input.setAttribute("aria-valuemax", String(max));
      const value = read(input.value);
      if (value !== null) input.setAttribute("aria-valuenow", String(value));
    }

    function clearError() {
      input.setCustomValidity("");
      input.removeAttribute("aria-invalid");
    }

    function revert() {
      input.value = committed;
      clearError();
      refreshAccessibility();
    }

    function commit(reportError = false) {
      const value = read(input.value);
      if (value === null || input.disabled || input.readOnly) {
        if (reportError && !input.disabled && !input.readOnly) {
          input.setCustomValidity(integer ? "Enter a whole number within the displayed bounds." : "Enter a finite number within the displayed bounds.");
          input.setAttribute("aria-invalid", "true");
          input.reportValidity();
        } else {
          revert();
        }
        return;
      }
      const changed = value !== read(committed);
      input.value = String(value);
      committed = input.value;
      clearError();
      refreshAccessibility();
      if (!changed) return;
      for (const name of ["input", "change"]) {
        const event = new Event(name, { bubbles: true });
        forwarded.add(event);
        input.dispatchEvent(event);
      }
      // Consumers may normalize through their authoritative state during input.
      committed = input.value;
      refreshAccessibility();
    }

    input.addEventListener("focus", () => {
      // Presets, reset and camera dragging write the same original input node.
      committed = input.value;
      clearError();
      refreshAccessibility();
    });
    for (const name of ["input", "change"]) {
      input.addEventListener(name, (event) => {
        if (forwarded.has(event)) return;
        event.stopImmediatePropagation();
        clearError();
      }, true);
    }
    input.addEventListener("blur", () => commit());
    input.addEventListener("keydown", (event) => {
      if (event.isComposing || !["Enter", "Escape", "ArrowUp", "ArrowDown"].includes(event.key)) return;
      event.preventDefault();
      event.stopPropagation();
      if (input.disabled || input.readOnly) return;
      if (event.key === "Escape") return revert();
      if (event.key === "Enter") return commit(true);
      const value = read(input.value);
      if (value === null) return commit(true);
      const increment = fineStep * (event.shiftKey ? 10 : 1) * (event.key === "ArrowUp" ? 1 : -1);
      const [min, max] = bounds();
      input.value = String(Math.max(min, Math.min(max, nudge(value, increment))));
      commit();
    });
    refreshAccessibility();
  }

  function install(root) {
    for (const input of root.querySelectorAll('.range-control input[type="range"]')) {
      attach(input, integerIds.has(input.id));
    }
  }

  if (typeof document !== "undefined") install(document);
  if (typeof module !== "undefined") module.exports = { parseDraft, nudge, attach, install };
})();
