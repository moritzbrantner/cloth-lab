const { test } = require("node:test");
const assert = require("node:assert/strict");
const { parseDraft, nudge, attach, install } = require("../site/precision-controls.js");

class Input extends EventTarget {
  constructor(value, { min = "-20", max = "5", step = "0.1", id = "gravity-control" } = {}) {
    super();
    Object.assign(this, { value, min, max, step, id, type: "range", disabled: false, readOnly: false });
    this.attributes = new Map([["value", value]]);
    this.classList = { add() {} };
  }
  getAttribute(name) { return this.attributes.get(name) ?? null; }
  setAttribute(name, value) { this.attributes.set(name, value); }
  removeAttribute(name) { this.attributes.delete(name); }
  setCustomValidity(message) { this.validationMessage = message; }
  reportValidity() { return !this.validationMessage; }
}

function key(input, key, extra = {}) {
  const event = new Event("keydown", { cancelable: true });
  Object.assign(event, { key, shiftKey: false, ...extra });
  input.dispatchEvent(event);
}
function edit(input, draft) {
  input.value = draft;
  input.dispatchEvent(new Event("input"));
}
function fixture(integer = false, options = {}) {
  const input = new Input(integer ? "12" : "-9.81", integer ? { min: "2", max: "24", step: "1", ...options } : options);
  attach(input, integer);
  const events = [];
  for (const name of ["input", "change"]) input.addEventListener(name, () => events.push([name, input.value]));
  input.dispatchEvent(new Event("focus"));
  return { input, events };
}

test("exact fractions, negative values, decimal comma and exponent notation are accepted without quantization", () => {
  for (const [raw, expected] of [["-9.80665", -9.80665], ["-9,81", -9.81], [".125", .125], ["1e-3", .001]]) {
    assert.equal(parseDraft(raw, -20, 5), expected);
  }
  assert.equal(parseDraft("5", -20, 5), 5);
  assert.equal(parseDraft("-20", -20, 5), -20);
});

test("invalid drafts, nonfinite values, out-of-bounds and fractional counts fail closed", () => {
  for (const raw of ["", " ", "-", ".", "1e", "NaN", "Infinity", "1e309", "0x10", "6", "-21", "1,2,3"]) {
    assert.equal(parseDraft(raw, -20, 5), null, raw);
  }
  assert.equal(parseDraft("2.5", 2, 24, true), null);
  assert.equal(parseDraft("12", 2, 24, true), 12);
});

test("restores the declared gravity instead of the browser-sanitized range value", () => {
  const input = new Input("-9.81");
  input.value = "-9.8";
  attach(input);
  assert.equal(input.type, "text");
  assert.equal(input.value, "-9.81");
  assert.equal(input.getAttribute("role"), "spinbutton");
});

test("drafts are isolated; Enter forwards one exact commit and subsequent blur does not duplicate it", () => {
  const { input, events } = fixture();
  for (const draft of ["", "-", "-9", "-9.", "-9.80665"]) edit(input, draft);
  assert.deepEqual(events, []);
  key(input, "Enter");
  input.dispatchEvent(new Event("blur"));
  assert.deepEqual(events, [["input", "-9.80665"], ["change", "-9.80665"]]);
});

test("Escape cancels valid drafts without leaking a value; invalid blur restores the committed value", () => {
  const { input, events } = fixture();
  edit(input, "-3.125");
  key(input, "Escape");
  assert.equal(input.value, "-9.81");
  edit(input, "");
  input.dispatchEvent(new Event("blur"));
  assert.equal(input.value, "-9.81");
  assert.deepEqual(events, []);
});

test("invalid Enter keeps the draft and exposes validation; editing clears it", () => {
  const { input, events } = fixture();
  edit(input, "99");
  key(input, "Enter");
  assert.equal(input.value, "99");
  assert.equal(input.getAttribute("aria-invalid"), "true");
  assert.deepEqual(events, []);
  edit(input, "-3.25");
  assert.equal(input.getAttribute("aria-invalid"), null);
  input.dispatchEvent(new Event("blur"));
  assert.equal(events[0][1], "-3.25");
});

test("fine keyboard nudges, Shift nudges, and integer boundaries remain exact", () => {
  assert.equal(nudge(.1, .2), .3);
  const { input } = fixture();
  key(input, "ArrowUp");
  assert.equal(input.value, "-9.8");
  key(input, "ArrowDown", { shiftKey: true });
  assert.equal(input.value, "-9.9");
  const count = fixture(true);
  edit(count.input, "24");
  key(count.input, "ArrowUp");
  assert.equal(count.input.value, "24");
  key(count.input, "ArrowDown");
  assert.equal(count.input.value, "23");
});

test("external reset values become the new cancellation baseline without a second authority", () => {
  const { input, events } = fixture();
  input.value = "-4.125";
  input.dispatchEvent(new Event("focus"));
  edit(input, "-2");
  key(input, "Escape");
  assert.equal(input.value, "-4.125");
  assert.deepEqual(events, []);
});

test("disabled, read-only and composition events do not commit", () => {
  for (const property of ["disabled", "readOnly"]) {
    const { input, events } = fixture();
    input[property] = true;
    key(input, "ArrowUp");
    assert.deepEqual(events, []);
  }
  const { input, events } = fixture();
  edit(input, "-3");
  key(input, "Enter", { isComposing: true });
  assert.deepEqual(events, []);
});

test("installation targets parameter controls, not the playback timeline", () => {
  const input = new Input("12", { id: "solver-iterations", min: "2", max: "24", step: "1" });
  install({ querySelectorAll(selector) {
    assert.equal(selector, '.range-control input[type="range"]');
    return [input];
  } });
  assert.equal(input.inputMode, "numeric");
});

test("authoritative consumer normalization does not cause a second commit on blur", () => {
  const { input, events } = fixture();
  input.addEventListener("input", () => { input.value = "-3.1250001"; });
  edit(input, "-3.125");
  key(input, "Enter");
  input.dispatchEvent(new Event("blur"));
  assert.equal(input.value, "-3.1250001");
  assert.equal(events.length, 2);
});

test("camera synchronization preserves fractions and editing a peer does not quantize rotation", () => {
  const { readFileSync } = require("node:fs");
  const { runInNewContext } = require("node:vm");
  const controls = new Map();
  const document = { querySelector(selector) {
    if (!controls.has(selector)) controls.set(selector, new Input("0"));
    return controls.get(selector);
  } };
  const noop = () => {};
  const scope = {
    document,
    canvas: { addEventListener: noop, removeEventListener: noop },
    interactionMode: { addEventListener: noop },
    moveObstacleDrag: noop,
    project: noop,
    pointerTarget: noop,
    drawFrame: noop,
  };
  const source = readFileSync(require("node:path").join(__dirname, "../site/camera.js"), "utf8");
  runInNewContext(source + `
    cameraYaw = radians(-36.125);
    cameraPitch = radians(33.456);
    cameraZoom = 1.234567;
    cameraPanX = 0.123456;
    cameraPanY = -0.234567;
    syncCameraControls();
    globalThis.synced = [cameraYawControl.value, cameraPitchControl.value,
      cameraZoomControl.value, cameraPanXControl.value, cameraPanYControl.value];
    cameraYawControl.value = "-36.125";
    cameraPitchControl.value = "33.456";
    cameraZoomControl.value = "1.234567";
    applyCameraControls();
    cameraPanXControl.value = "0.012345";
    applyCameraControls();
    globalThis.after = [cameraYawControl.value, cameraZoomControl.value,
      cameraPanXControl.value, degrees(cameraYaw), cameraZoom];
  `, scope);
  assert.ok(Math.abs(Number(scope.synced[0]) + 36.125) < 1e-12);
  assert.ok(Math.abs(Number(scope.synced[1]) - 33.456) < 1e-12);
  assert.deepEqual(Array.from(scope.synced).slice(2), ["1.234567", "0.123456", "-0.234567"]);
  assert.deepEqual(Array.from(scope.after).slice(0, 3), ["-36.125", "1.234567", "0.012345"]);
  assert.ok(Math.abs(scope.after[3] + 36.125) < 1e-12);
  assert.equal(scope.after[4], 1.234567);
});
