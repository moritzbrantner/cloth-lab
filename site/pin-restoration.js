"use strict";

(function (root) {
  function validIndex(value) {
    return Number.isInteger(value) && value >= 0;
  }

  function validTarget(value) {
    return Array.isArray(value) && value.length === 3 && value.every(Number.isFinite);
  }

  function planPinRestoration(existingPinned, existingAttached, savedPins) {
    if (
      !Array.isArray(existingPinned) ||
      !existingPinned.every(validIndex) ||
      !Array.isArray(existingAttached) ||
      !existingAttached.every(validIndex) ||
      !Array.isArray(savedPins)
    ) {
      throw new Error("pin restoration metadata is invalid");
    }

    const attached = new Set(existingAttached);
    const saved = new Map();
    for (const pin of savedPins) {
      if (
        !pin ||
        !validIndex(pin.index) ||
        !validTarget(pin.target) ||
        typeof pin.attached !== "boolean" ||
        saved.has(pin.index)
      ) {
        throw new Error("saved pin metadata is invalid");
      }
      saved.set(pin.index, pin);
    }

    const operations = [];
    for (const index of existingPinned) {
      const pin = saved.get(index);
      if (!pin) {
        operations.push({ kind: "remove", index });
        continue;
      }
      saved.delete(index);
      if (pin.attached) {
        if (!attached.has(index)) {
          throw new Error("saved mannequin attachment is unavailable in rebuilt template");
        }
        operations.push({ kind: "keep-attached", index });
      } else {
        operations.push({ kind: "replace-detached", index, target: [...pin.target] });
      }
    }

    for (const pin of saved.values()) {
      if (pin.attached) {
        throw new Error("saved mannequin attachment is unavailable in rebuilt template");
      }
      operations.push({ kind: "add-detached", index: pin.index, target: [...pin.target] });
    }
    return operations;
  }

  root.planPinRestoration = planPinRestoration;
  if (typeof module !== "undefined") {
    module.exports = { planPinRestoration };
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
