"use strict";

const canvas = document.querySelector("#cloth-canvas");
const context = canvas.getContext("2d");
const playToggle = document.querySelector("#play-toggle");
const singleStepButton = document.querySelector("#single-step");
const restartButton = document.querySelector("#restart");
const useDemoButton = document.querySelector("#use-demo");
const clearPinsButton = document.querySelector("#clear-pins");
const slider = document.querySelector("#frame-slider");
const timeline = document.querySelector(".timeline");
const status = document.querySelector("#status");
const garmentUpload = document.querySelector("#garment-upload");
const materialPreset = document.querySelector("#material-preset");
const meshResolution = document.querySelector("#mesh-resolution");
const meshResolutionValue = document.querySelector("#mesh-resolution-value");
const gravityControl = document.querySelector("#gravity-control");
const gravityValue = document.querySelector("#gravity-value");
const solverIterations = document.querySelector("#solver-iterations");
const solverIterationsValue = document.querySelector("#solver-iterations-value");
const velocityDamping = document.querySelector("#velocity-damping");
const velocityDampingValue = document.querySelector("#velocity-damping-value");
const interactionMode = document.querySelector("#interaction-mode");

const YAW = -0.62;
const PITCH = 0.58;
const FIXED_STEP_MS = 1000 / 60;
const MAX_STEPS_PER_FRAME = 4;
const DEMO_ROWS_NUMERATOR = 11;
const DEMO_ROWS_DENOMINATOR = 13;

let snapshots = null;
let frameIndex = 0;
let playing = true;
let lastAdvance = 0;
let liveAccumulator = 0;
let projection = null;
let wasmModulePromise = null;
let liveSession = null;
let livePositions = [];
let liveTriangles = [];
let livePinned = [];
let liveCapsule = null;
let liveStepCount = 0;
let liveFingerprint = "";
let liveSourceLabel = "Sheet demo";
let currentUpload = null;
let dragState = null;

function includePoint(bounds, [x, y, z]) {
  bounds.minX = Math.min(bounds.minX, x);
  bounds.minY = Math.min(bounds.minY, y);
  bounds.minZ = Math.min(bounds.minZ, z);
  bounds.maxX = Math.max(bounds.maxX, x);
  bounds.maxY = Math.max(bounds.maxY, y);
  bounds.maxZ = Math.max(bounds.maxZ, z);
}

function includeRadius(bounds, point, radius) {
  const [x, y, z] = point;
  includePoint(bounds, [x - radius, y - radius, z - radius]);
  includePoint(bounds, [x + radius, y + radius, z + radius]);
}

function computeProjectionFromPositions(positions, capsule = null) {
  const bounds = {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
  };

  for (const position of positions) {
    includePoint(bounds, position);
  }

  if (capsule) {
    const radius = capsule.radius + capsule.thickness;
    includeRadius(bounds, capsule.start, radius);
    includeRadius(bounds, capsule.end, radius);
  }

  const center = [
    (bounds.minX + bounds.maxX) / 2,
    (bounds.minY + bounds.maxY) / 2,
    (bounds.minZ + bounds.maxZ) / 2,
  ];
  const span = Math.max(
    bounds.maxX - bounds.minX,
    bounds.maxY - bounds.minY,
    bounds.maxZ - bounds.minZ,
    0.001,
  );

  return { center, span };
}

function computeSnapshotProjection(data) {
  const positions = data.frames.flatMap((frame) => frame.positions);
  return computeProjectionFromPositions(positions, data.capsule);
}

function resizeCanvas() {
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
  const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
}

function project(position) {
  const [centerX, centerY, centerZ] = projection.center;
  const x = position[0] - centerX;
  const y = position[1] - centerY;
  const z = position[2] - centerZ;

  const yawCos = Math.cos(YAW);
  const yawSin = Math.sin(YAW);
  const pitchCos = Math.cos(PITCH);
  const pitchSin = Math.sin(PITCH);

  const rotatedX = yawCos * x - yawSin * z;
  const yawDepth = yawSin * x + yawCos * z;
  const rotatedY = pitchCos * y - pitchSin * yawDepth;
  const depth = pitchSin * y + pitchCos * yawDepth;

  const baseScale = Math.min(canvas.width, canvas.height) / (projection.span * 1.55);
  const perspective = 1 / Math.max(0.68, 1 + depth / (projection.span * 4.5));

  return {
    x: canvas.width / 2 + rotatedX * baseScale * perspective,
    y: canvas.height / 2 - rotatedY * baseScale * perspective,
    depth,
    scale: baseScale * perspective,
  };
}

function projectedRadius(center, radius) {
  const projectedCenter = project(center);
  const projectedX = project([center[0] + radius, center[1], center[2]]);
  const projectedY = project([center[0], center[1] + radius, center[2]]);
  return Math.max(
    Math.hypot(projectedX.x - projectedCenter.x, projectedX.y - projectedCenter.y),
    Math.hypot(projectedY.x - projectedCenter.x, projectedY.y - projectedCenter.y),
  );
}

function drawCapsule(capsule) {
  const start = project(capsule.start);
  const end = project(capsule.end);
  const midpoint = capsule.start.map((value, index) => (value + capsule.end[index]) / 2);
  const physicalRadius = projectedRadius(midpoint, capsule.radius);
  const shellRadius = projectedRadius(midpoint, capsule.radius + capsule.thickness);

  context.save();
  context.lineCap = "round";

  context.beginPath();
  context.moveTo(start.x, start.y);
  context.lineTo(end.x, end.y);
  context.strokeStyle = "rgba(240, 195, 109, 0.2)";
  context.lineWidth = shellRadius * 2;
  context.stroke();

  context.beginPath();
  context.moveTo(start.x, start.y);
  context.lineTo(end.x, end.y);
  context.strokeStyle = "rgba(67, 88, 112, 0.96)";
  context.lineWidth = physicalRadius * 2;
  context.stroke();

  context.beginPath();
  context.moveTo(start.x, start.y);
  context.lineTo(end.x, end.y);
  context.setLineDash([Math.max(5, canvas.width / 240), Math.max(5, canvas.width / 240)]);
  context.strokeStyle = "rgba(205, 220, 236, 0.48)";
  context.lineWidth = Math.max(1, canvas.width / 1400);
  context.stroke();
  context.restore();
}

function drawSurface(positions, triangles, pinned = [], selectedIndex = null) {
  const projected = positions.map(project);
  const orderedTriangles = triangles
    .map((triangle) => ({
      triangle,
      depth:
        (projected[triangle[0]].depth +
          projected[triangle[1]].depth +
          projected[triangle[2]].depth) /
        3,
    }))
    .sort((a, b) => a.depth - b.depth);

  context.lineJoin = "round";
  for (const { triangle } of orderedTriangles) {
    const a = projected[triangle[0]];
    const b = projected[triangle[1]];
    const c = projected[triangle[2]];

    context.beginPath();
    context.moveTo(a.x, a.y);
    context.lineTo(b.x, b.y);
    context.lineTo(c.x, c.y);
    context.closePath();
    context.fillStyle = "rgba(90, 145, 205, 0.22)";
    context.fill();
    context.strokeStyle = "rgba(174, 205, 238, 0.34)";
    context.lineWidth = Math.max(1, canvas.width / 1500);
    context.stroke();
  }

  for (const pinnedIndex of pinned) {
    const point = projected[pinnedIndex];
    if (!point) {
      continue;
    }
    context.beginPath();
    context.arc(point.x, point.y, Math.max(4, canvas.width / 260), 0, Math.PI * 2);
    context.fillStyle = "#f0c36d";
    context.fill();
  }

  if (selectedIndex !== null) {
    const point = projected[selectedIndex];
    if (point) {
      context.beginPath();
      context.arc(point.x, point.y, Math.max(6, canvas.width / 210), 0, Math.PI * 2);
      context.strokeStyle = "#f0c36d";
      context.lineWidth = Math.max(2, canvas.width / 900);
      context.stroke();
    }
  }

  return projected;
}

function drawFrame() {
  if (!projection) {
    return;
  }

  resizeCanvas();
  context.clearRect(0, 0, canvas.width, canvas.height);

  if (liveSession) {
    if (liveCapsule) {
      drawCapsule(liveCapsule);
    }
    drawSurface(livePositions, liveTriangles, livePinned, dragState?.index ?? null);
    return;
  }

  if (!snapshots) {
    return;
  }

  const frame = snapshots.frames[frameIndex];
  drawCapsule(snapshots.capsule);
  drawSurface(frame.positions, snapshots.triangles, snapshots.pinned);
  slider.value = String(frameIndex);
  status.textContent = `${snapshots.materialPreset} · reference step ${frame.step} · fingerprint ${frame.fingerprint} · stretch ${frame.maxStretchError.toExponential(2)} · shear ${frame.maxShearError.toExponential(2)} · bend ${frame.maxBendingError.toExponential(2)} · contacts ${frame.collisionProjections}`;
}

function tick(timestamp) {
  if (liveSession) {
    if (playing) {
      if (lastAdvance === 0) {
        lastAdvance = timestamp;
      }
      const elapsed = Math.min(100, Math.max(0, timestamp - lastAdvance));
      lastAdvance = timestamp;
      liveAccumulator += elapsed;
      let stepped = false;
      let steps = 0;
      while (liveAccumulator >= FIXED_STEP_MS && steps < MAX_STEPS_PER_FRAME) {
        try {
          liveSession.step();
        } catch (error) {
          setPlaying(false);
          status.textContent = `Simulation stopped: ${formatError(error)}`;
          break;
        }
        liveAccumulator -= FIXED_STEP_MS;
        liveStepCount += 1;
        steps += 1;
        stepped = true;
      }
      if (steps === MAX_STEPS_PER_FRAME) {
        liveAccumulator = Math.min(liveAccumulator, FIXED_STEP_MS);
      }
      if (stepped) {
        refreshLiveGeometry(liveStepCount % 30 === 0);
      }
    } else {
      lastAdvance = timestamp;
      liveAccumulator = 0;
    }
    drawFrame();
  } else if (snapshots && playing && timestamp - lastAdvance >= 1000 / 30) {
    frameIndex = (frameIndex + 1) % snapshots.frames.length;
    lastAdvance = timestamp;
    drawFrame();
  }
  window.requestAnimationFrame(tick);
}

function setPlaying(nextPlaying) {
  playing = nextPlaying;
  playToggle.textContent = playing ? "Pause" : "Play";
  lastAdvance = 0;
  liveAccumulator = 0;
}

function flatPositionsToVectors(values) {
  const flat = Array.from(values);
  if (flat.length === 0 || flat.length % 3 !== 0) {
    throw new Error("Rust solver returned invalid position data");
  }
  const positions = [];
  for (let index = 0; index < flat.length; index += 3) {
    const position = [flat[index], flat[index + 1], flat[index + 2]];
    if (!position.every(Number.isFinite)) {
      throw new Error("Rust solver returned non-finite position data");
    }
    positions.push(position);
  }
  return positions;
}

function flatTrianglesToVectors(values) {
  const flat = Array.from(values);
  if (flat.length === 0 || flat.length % 3 !== 0) {
    throw new Error("Rust solver returned invalid triangle data");
  }
  const triangles = [];
  for (let index = 0; index < flat.length; index += 3) {
    triangles.push([flat[index], flat[index + 1], flat[index + 2]]);
  }
  return triangles;
}

function capsuleFromFlat(values) {
  const flat = Array.from(values);
  if (flat.length === 0) {
    return null;
  }
  if (flat.length !== 8 || !flat.every(Number.isFinite)) {
    throw new Error("Rust solver returned invalid capsule collider data");
  }
  return {
    start: flat.slice(0, 3),
    end: flat.slice(3, 6),
    radius: flat[6],
    thickness: flat[7],
  };
}

function refreshLivePins() {
  livePinned = Array.from(liveSession.pinnedIndices());
  clearPinsButton.disabled = livePinned.length === 0;
}

function refreshLiveGeometry(refreshFingerprint = false) {
  livePositions = flatPositionsToVectors(liveSession.positions());
  if (refreshFingerprint || liveFingerprint === "") {
    liveFingerprint = liveSession.fingerprint();
  }
  updateLiveStatus();
}

function refreshLiveTopology() {
  livePositions = flatPositionsToVectors(liveSession.positions());
  liveTriangles = flatTrianglesToVectors(liveSession.triangles());
  liveCapsule = capsuleFromFlat(liveSession.capsuleCollider());
  refreshLivePins();
  liveFingerprint = liveSession.fingerprint();
  updateLiveStatus();
}

function selectedMaterialLabel() {
  return materialPreset.options[materialPreset.selectedIndex]?.textContent ?? materialPreset.value;
}

function updateLiveStatus() {
  if (!liveSession) {
    return;
  }
  status.textContent = `${liveSourceLabel} · ${selectedMaterialLabel()} · ${liveSession.vertexCount()} vertices · ${liveSession.triangleCount()} faces · ${livePinned.length} pins · step ${liveStepCount} · fingerprint ${liveFingerprint}`;
}

async function loadWasmModule() {
  if (!wasmModulePromise) {
    wasmModulePromise = import("./pkg/cloth_lab.js").then(async (module) => {
      await module.default();
      return module;
    });
  }
  return wasmModulePromise;
}

function extensionOf(filename) {
  const dot = filename.lastIndexOf(".");
  return dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "";
}

function applyRuntimeControls(session) {
  session.setGravity(Number(gravityControl.value));
  session.setSolverIterations(Number(solverIterations.value));
  session.setVelocityDamping(Number(velocityDamping.value));
}

function capturePins() {
  if (!liveSession) {
    return null;
  }
  return livePinned.map((index) => ({ index, target: [...livePositions[index]] }));
}

function restorePins(session, pins) {
  if (pins === null) {
    return;
  }
  for (const index of Array.from(session.pinnedIndices())) {
    session.unpinParticle(index);
  }
  const vertexCount = session.vertexCount();
  for (const pin of pins) {
    if (pin.index < 0 || pin.index >= vertexCount || !pin.target.every(Number.isFinite)) {
      continue;
    }
    session.pinParticle(pin.index);
    session.movePin(pin.index, pin.target[0], pin.target[1], pin.target[2]);
  }
}

function activateSession(session, sourceLabel, upload, autoplay) {
  if (liveSession && typeof liveSession.free === "function") {
    liveSession.free();
  }
  liveSession = session;
  currentUpload = upload;
  liveSourceLabel = sourceLabel;
  liveStepCount = 0;
  liveFingerprint = "";
  dragState = null;
  refreshLiveTopology();
  projection = computeProjectionFromPositions(livePositions, liveCapsule);
  timeline.hidden = true;
  canvas.classList.add("interactive");
  slider.disabled = true;
  playToggle.disabled = false;
  singleStepButton.disabled = false;
  restartButton.disabled = false;
  useDemoButton.disabled = upload === null;
  meshResolution.disabled = upload !== null;
  setPlaying(autoplay);
  updateInteractionCursor();
  drawFrame();
}

async function activateDemo(pins = null, autoplay = true) {
  status.textContent = "Building interactive sheet…";
  const module = await loadWasmModule();
  const session = module.BrowserClothSession.fromDemo(
    Number(meshResolution.value),
    materialPreset.value,
  );
  applyRuntimeControls(session);
  restorePins(session, pins);
  activateSession(session, "Sheet demo", null, autoplay);
}

async function activateUpload(upload, pins = null) {
  status.textContent = `Loading ${upload.name}…`;
  const module = await loadWasmModule();
  let session;
  if (upload.extension === "obj") {
    session = module.BrowserClothSession.fromObj(upload.bytes, materialPreset.value);
  } else if (upload.extension === "glb") {
    session = module.BrowserClothSession.fromGlb(upload.bytes, materialPreset.value);
  } else {
    throw new Error("supported garment uploads are .obj and self-contained .glb files");
  }
  applyRuntimeControls(session);
  restorePins(session, pins);
  activateSession(session, upload.name, upload, false);
}

function formatError(error) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function canvasPoint(event) {
  const bounds = canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - bounds.left) / bounds.width) * canvas.width,
    y: ((event.clientY - bounds.top) / bounds.height) * canvas.height,
  };
}

function nearestLiveParticle(point) {
  let bestIndex = null;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let index = 0; index < livePositions.length; index += 1) {
    const projected = project(livePositions[index]);
    const distance = Math.hypot(projected.x - point.x, projected.y - point.y);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = index;
    }
  }
  const threshold = Math.max(14 * Math.min(window.devicePixelRatio || 1, 2), canvas.width / 70);
  return bestDistance <= threshold ? bestIndex : null;
}

function startPointerManipulation(event, index, kind) {
  const point = canvasPoint(event);
  const origin = [...livePositions[index]];
  const projected = project(origin);
  dragState = {
    kind,
    index,
    pointerId: event.pointerId,
    startX: point.x,
    startY: point.y,
    origin,
    scale: Math.max(projected.scale, 1e-9),
  };
  canvas.setPointerCapture(event.pointerId);
  canvas.classList.add("dragging");
  event.preventDefault();
  drawFrame();
}

function beginCanvasDrag(event) {
  if (!liveSession || dragState) {
    return;
  }
  const point = canvasPoint(event);
  const index = nearestLiveParticle(point);
  if (index === null) {
    return;
  }

  const mode = interactionMode.value;
  if (mode === "unpin") {
    if (!livePinned.includes(index)) {
      status.textContent = "That vertex is not pinned.";
      return;
    }
    try {
      liveSession.unpinParticle(index);
      refreshLivePins();
      liveFingerprint = liveSession.fingerprint();
      updateLiveStatus();
      drawFrame();
    } catch (error) {
      status.textContent = `Could not remove pin: ${formatError(error)}`;
    }
    event.preventDefault();
    return;
  }

  if (mode === "pin") {
    try {
      if (!livePinned.includes(index)) {
        liveSession.pinParticle(index);
        refreshLivePins();
      }
      startPointerManipulation(event, index, "pin");
    } catch (error) {
      status.textContent = `Could not place pin: ${formatError(error)}`;
    }
    return;
  }

  try {
    liveSession.beginDrag(index);
    startPointerManipulation(event, index, "drag");
  } catch (error) {
    status.textContent = livePinned.includes(index)
      ? "That vertex is pinned. Switch the pointer tool to Add / move pins or Remove pins."
      : `Cannot drag vertex: ${formatError(error)}`;
  }
}

function pointerTarget(event) {
  const point = canvasPoint(event);
  const dx = (point.x - dragState.startX) / dragState.scale;
  const dy = (point.y - dragState.startY) / dragState.scale;
  const yawCos = Math.cos(YAW);
  const yawSin = Math.sin(YAW);
  const pitchCos = Math.cos(PITCH);
  const pitchSin = Math.sin(PITCH);
  const right = [yawCos, 0, -yawSin];
  const up = [-yawSin * pitchSin, pitchCos, -yawCos * pitchSin];
  return dragState.origin.map((value, axis) => value + right[axis] * dx - up[axis] * dy);
}

function updateCanvasDrag(event) {
  if (!liveSession || !dragState || event.pointerId !== dragState.pointerId) {
    return;
  }

  const target = pointerTarget(event);
  try {
    if (dragState.kind === "pin") {
      liveSession.movePin(dragState.index, target[0], target[1], target[2]);
    } else {
      liveSession.dragTo(target[0], target[1], target[2]);
    }
    refreshLiveGeometry(false);
    drawFrame();
  } catch (error) {
    status.textContent = `Pointer interaction stopped: ${formatError(error)}`;
    finishCanvasDrag(event);
  }
  event.preventDefault();
}

function finishCanvasDrag(event) {
  if (!liveSession || !dragState || event.pointerId !== dragState.pointerId) {
    return;
  }
  try {
    if (dragState.kind === "drag") {
      liveSession.endDrag();
    }
    liveFingerprint = liveSession.fingerprint();
    updateLiveStatus();
  } catch (error) {
    status.textContent = `Pointer release failed: ${formatError(error)}`;
  }
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
  dragState = null;
  canvas.classList.remove("dragging");
  event.preventDefault();
  drawFrame();
}

function rowsForResolution(resolution) {
  return (
    Math.floor(
      ((resolution - 1) * DEMO_ROWS_NUMERATOR + Math.floor(DEMO_ROWS_DENOMINATOR / 2)) /
        DEMO_ROWS_DENOMINATOR,
    ) + 1
  );
}

function faceCountForResolution(resolution) {
  const rows = rowsForResolution(resolution);
  return 2 * (resolution - 1) * (rows - 1);
}

function updateControlLabels() {
  const resolution = Number(meshResolution.value);
  meshResolutionValue.textContent = `${faceCountForResolution(resolution)} faces`;
  gravityValue.textContent = `${Number(gravityControl.value).toFixed(2)} m/s²`;
  solverIterationsValue.textContent = solverIterations.value;
  velocityDampingValue.textContent = Number(velocityDamping.value).toFixed(3);
}

function updateInteractionCursor() {
  canvas.classList.toggle("pin-tool", interactionMode.value === "pin");
  canvas.classList.toggle("unpin-tool", interactionMode.value === "unpin");
}

function validateSnapshots(data) {
  if (!Array.isArray(data.frames) || data.frames.length === 0) {
    throw new Error("snapshot payload has no frames");
  }
  if (
    !data.capsule ||
    !Array.isArray(data.capsule.start) ||
    !Array.isArray(data.capsule.end)
  ) {
    throw new Error("snapshot payload has no capsule collider metadata");
  }
  if (typeof data.materialPreset !== "string" || data.materialPreset.length === 0) {
    throw new Error("snapshot payload has no material preset evidence");
  }
  if (
    !data.materialParameters ||
    !Number.isFinite(data.materialParameters.stretchCompliance) ||
    !Number.isFinite(data.materialParameters.shearCompliance) ||
    !Number.isFinite(data.materialParameters.bendingCompliance) ||
    !Number.isFinite(data.materialParameters.frictionCoefficient)
  ) {
    throw new Error("snapshot payload has no raw material parameter evidence");
  }
}

playToggle.addEventListener("click", () => {
  setPlaying(!playing);
});

singleStepButton.addEventListener("click", () => {
  setPlaying(false);
  if (liveSession) {
    try {
      liveSession.step();
      liveStepCount += 1;
      refreshLiveGeometry(true);
      drawFrame();
    } catch (error) {
      status.textContent = `Simulation step failed: ${formatError(error)}`;
    }
    return;
  }
  if (snapshots) {
    frameIndex = (frameIndex + 1) % snapshots.frames.length;
    drawFrame();
  }
});

restartButton.addEventListener("click", () => {
  if (liveSession) {
    try {
      liveSession.reset();
      liveStepCount = 0;
      liveFingerprint = "";
      dragState = null;
      canvas.classList.remove("dragging");
      refreshLiveTopology();
      projection = computeProjectionFromPositions(livePositions, liveCapsule);
      setPlaying(true);
      drawFrame();
    } catch (error) {
      status.textContent = `Simulation reset failed: ${formatError(error)}`;
    }
    return;
  }
  frameIndex = 0;
  setPlaying(true);
  drawFrame();
});

slider.addEventListener("input", () => {
  if (liveSession) {
    return;
  }
  frameIndex = Number(slider.value);
  setPlaying(false);
  drawFrame();
});

garmentUpload.addEventListener("change", async () => {
  const file = garmentUpload.files?.[0];
  if (!file) {
    return;
  }
  const extension = extensionOf(file.name);
  if (extension !== "obj" && extension !== "glb") {
    status.textContent = "Unsupported garment file. Choose an OBJ or self-contained GLB file.";
    return;
  }

  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const upload = { name: file.name, extension, bytes };
    await activateUpload(upload);
  } catch (error) {
    status.textContent = `Garment import failed: ${formatError(error)}`;
  }
});

useDemoButton.addEventListener("click", async () => {
  try {
    await activateDemo();
  } catch (error) {
    status.textContent = `Could not restore sheet demo: ${formatError(error)}`;
  }
});

materialPreset.addEventListener("change", async () => {
  const pins = capturePins();
  try {
    if (currentUpload) {
      await activateUpload(currentUpload, pins);
    } else {
      await activateDemo(pins, playing);
    }
  } catch (error) {
    status.textContent = `Could not apply textile preset: ${formatError(error)}`;
  }
});

meshResolution.addEventListener("input", updateControlLabels);
meshResolution.addEventListener("change", async () => {
  if (currentUpload) {
    return;
  }
  try {
    await activateDemo(null, playing);
  } catch (error) {
    status.textContent = `Could not rebuild sheet mesh: ${formatError(error)}`;
  }
});

gravityControl.addEventListener("input", () => {
  updateControlLabels();
  if (!liveSession) {
    return;
  }
  try {
    liveSession.setGravity(Number(gravityControl.value));
  } catch (error) {
    status.textContent = `Could not change gravity: ${formatError(error)}`;
  }
});

solverIterations.addEventListener("input", () => {
  updateControlLabels();
  if (!liveSession) {
    return;
  }
  try {
    liveSession.setSolverIterations(Number(solverIterations.value));
  } catch (error) {
    status.textContent = `Could not change solver iterations: ${formatError(error)}`;
  }
});

velocityDamping.addEventListener("input", () => {
  updateControlLabels();
  if (!liveSession) {
    return;
  }
  try {
    liveSession.setVelocityDamping(Number(velocityDamping.value));
  } catch (error) {
    status.textContent = `Could not change velocity damping: ${formatError(error)}`;
  }
});

interactionMode.addEventListener("change", () => {
  updateInteractionCursor();
  drawFrame();
});

clearPinsButton.addEventListener("click", () => {
  if (!liveSession || dragState) {
    return;
  }
  try {
    for (const index of [...livePinned]) {
      liveSession.unpinParticle(index);
    }
    refreshLivePins();
    liveFingerprint = liveSession.fingerprint();
    updateLiveStatus();
    drawFrame();
  } catch (error) {
    status.textContent = `Could not clear pins: ${formatError(error)}`;
  }
});

canvas.addEventListener("pointerdown", beginCanvasDrag);
canvas.addEventListener("pointermove", updateCanvasDrag);
canvas.addEventListener("pointerup", finishCanvasDrag);
canvas.addEventListener("pointercancel", finishCanvasDrag);
window.addEventListener("resize", drawFrame);

updateControlLabels();
updateInteractionCursor();

fetch("frames.json")
  .then((response) => {
    if (!response.ok) {
      throw new Error(`snapshot request failed with ${response.status}`);
    }
    return response.json();
  })
  .then((data) => {
    validateSnapshots(data);
    snapshots = data;
    slider.max = String(data.frames.length - 1);
    if (!liveSession) {
      projection = computeSnapshotProjection(data);
      slider.disabled = false;
      playToggle.disabled = false;
      singleStepButton.disabled = false;
      restartButton.disabled = false;
      drawFrame();
    }
  })
  .catch((error) => {
    if (!liveSession) {
      status.textContent = `Reference demo unavailable: ${formatError(error)}`;
    }
  });

activateDemo()
  .catch((error) => {
    status.textContent = `Interactive demo unavailable: ${formatError(error)}. Falling back to reference snapshots.`;
  });

window.requestAnimationFrame(tick);
