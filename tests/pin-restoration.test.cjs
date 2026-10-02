"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

const { planPinRestoration } = require("../site/pin-restoration.js");

const target = (x) => [x, x + 1, x + 2];

test("keeps semantic mannequin attachments intact across rebuilds", () => {
  assert.deepEqual(
    planPinRestoration(
      [3, 9],
      [3, 9],
      [
        { index: 3, target: target(1), attached: true },
        { index: 9, target: target(2), attached: true },
      ],
    ),
    [
      { kind: "keep-attached", index: 3 },
      { kind: "keep-attached", index: 9 },
    ],
  );
});

test("preserves an explicitly detached authored pin as detached", () => {
  assert.deepEqual(
    planPinRestoration(
      [3, 9],
      [3, 9],
      [
        { index: 3, target: target(4), attached: false },
        { index: 9, target: target(2), attached: true },
      ],
    ),
    [
      { kind: "replace-detached", index: 3, target: target(4) },
      { kind: "keep-attached", index: 9 },
    ],
  );
});

test("removes deleted authored pins and adds user pins deterministically", () => {
  assert.deepEqual(
    planPinRestoration(
      [3, 9],
      [3, 9],
      [
        { index: 9, target: target(2), attached: true },
        { index: 12, target: target(7), attached: false },
      ],
    ),
    [
      { kind: "remove", index: 3 },
      { kind: "keep-attached", index: 9 },
      { kind: "add-detached", index: 12, target: target(7) },
    ],
  );
});

test("fails closed when an attached pin cannot be restored semantically", () => {
  assert.throws(
    () =>
      planPinRestoration(
        [],
        [],
        [{ index: 3, target: target(1), attached: true }],
      ),
    /saved mannequin attachment is unavailable/,
  );
});
