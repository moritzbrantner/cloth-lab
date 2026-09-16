"use strict";

const obstacleKindControl = document.querySelector("#obstacle-kind");
const obstacleXControl = document.querySelector("#obstacle-x");
const obstacleYControl = document.querySelector("#obstacle-y");
const obstacleZControl = document.querySelector("#obstacle-z");
const obstacleRadiusControl = document.querySelector("#obstacle-radius");
const obstacleThicknessControl = document.querySelector("#obstacle-thickness");
const obstacleLengthControl = document.querySelector("#obstacle-length");
const obstacleXValue = document.querySelector("#obstacle-x-value");
const obstacleYValue = document.querySelector("#obstacle-y-value");
const obstacleZValue = document.querySelector("#obstacle-z-value");
const obstacleRadiusValue = document.querySelector("#obstacle-radius-value");
const obstacleThicknessValue = document.querySelector("#obstacle-thickness-value");
const obstacleLengthValue = document.querySelector("#obstacle-length-value");
const inspectObstacle = document.querySelector("#inspect-obstacle");

const OBSTACLE_POINTER_PADDING = 16;
let liveObstacle = null;
let obstacleDragState = null;

function obstacleFromFlat(values) {
  const flat = Array.from(values);
  if (flat.length === 1 && flat[0] === 0) {
    return null;
  }
  if (flat.length !== 7 || !flat.every(Number.isFinite)) {
    throw new Error("Rust solver returned invalid obstacle data");
  }
  const kind = flat[0] === 1 ? "sphere" : flat[0] === 2 ? "capsule" : null;
  if (!kind || flat[4] <= 0 || flat[5] < 0 || (kind === "capsule" && flat[6] <= 0)) {
    throw new Error("Rust solver returned invalid obstacle configuration");
  }
  return {
    kind,
    center: flat.slice(1, 4),
    radius: flat[4],
    thickness: flat[5],
    halfLength: flat[6],
  };
}

function clampToControl(control, value) {
  const minimum = Number(control.min);
  const maximum = Number(control.max);
  return Math.min(maximum, Math.max(minimum, value));
}

function setControlValue(control, value) {
  control.value = String(clampToControl(control, value));
}

function updateObstacleLabels() {
  obstacleXValue.textContent = Number(obstacleXControl.value).toFixed(2);
  obstacleYValue.textContent = Number(obstacleYControl.value).toFixed(2);
  obstacleZValue.textContent = Number(obstacleZControl.value).toFixed(2);
  obstacleRadiusValue.textContent = Number(obstacleRadiusControl.value).toFixed(2);
  obstacleThicknessValue.textContent = Number(obstacleThicknessControl.value).toFixed(3);
  obstacleLengthValue.textContent = Number(obstacleLengthControl.value).toFixed(2);
}

function updateObstacleControlState() {
  const enabled = obstacleKindControl.value !== "none";
  for (const control of [
    obstacleXControl,
    obstacleYControl,
    obstacleZControl,
    obstacleRadiusControl,
    obstacleThicknessControl,
  ]) {
    control.disabled = !enabled;
  }
  obstacleLengthControl.disabled = obstacleKindControl.value !== "capsule";
}

function obstacleCapsuleDescriptor(obstacle) {
  if (!obstacle || obstacle.kind !== "capsule") {
    return null;
  }
  return {
    start: [
      obstacle.center[0] - obstacle.halfLength,
      obstacle.center[1],
      obstacle.center[2],
    ],
    end: [
      obstacle.center[0] + obstacle.halfLength,
      obstacle.center[1],
      obstacle.center[2],
    ],
    radius: obstacle.radius,
    thickness: obstacle.thickness,
  };
}

function updateObstacleInspector() {
  if (!inspectObstacle) {
    return;
  }
  if (!liveObstacle) {
    inspectObstacle.textContent = "none";
    return;
  }
  const position = liveObstacle.center.map((value) => value.toFixed(2)).join(", ");
  const size =
    liveObstacle.kind === "capsule"
      ? `r ${liveObstacle.radius.toFixed(2)} · length ${(liveObstacle.halfLength * 2).toFixed(2)}`
      : `r ${liveObstacle.radius.toFixed(2)}`;
  inspectObstacle.textContent = `${liveObstacle.kind} · (${position}) · ${size}`;
}

function syncObstacleFromSession() {
  if (!liveSession) {
    liveObstacle = null;
    updateObstacleInspector();
    return;
  }

  liveObstacle = obstacleFromFlat(liveSession.obstacleDescriptor());
  if (liveObstacle) {
    obstacleKindControl.value = liveObstacle.kind;
    setControlValue(obstacleXControl, liveObstacle.center[0]);
    setControlValue(obstacleYControl, liveObstacle.center[1]);
    setControlValue(obstacleZControl, liveObstacle.center[2]);
    setControlValue(obstacleRadiusControl, liveObstacle.radius);
    setControlValue(obstacleThicknessControl, liveObstacle.thickness);
    if (liveObstacle.kind === "capsule") {
      setControlValue(obstacleLengthControl, liveObstacle.halfLength * 2);
    }
  } else {
    obstacleKindControl.value = "none";
  }
  liveCapsule = obstacleCapsuleDescriptor(liveObstacle);
  updateObstacleControlState();
  updateObstacleLabels();
  updateObstacleInspector();
}

function captureObstacleConfig() {
  if (!liveSession) {
    return null;
  }
  return {
    kind: liveObstacle?.kind ?? "none",
    center: liveObstacle
      ? [...liveObstacle.center]
      : [
          Number(obstacleXControl.value),
          Number(obstacleYControl.value),
          Number(obstacleZControl.value),
        ],
    radius: liveObstacle?.radius ?? Number(obstacleRadiusControl.value),
    thickness: liveObstacle?.thickness ?? Number(obstacleThicknessControl.value),
    halfLength: liveObstacle?.halfLength ?? Number(obstacleLengthControl.value) / 2,
  };
}

function restoreObstacleConfig(config) {
  if (!config || !liveSession) {
    return;
  }
  liveSession.setObstacle(
    config.kind,
    config.center[0],
    config.center[1],
    config.center[2],
    config.radius,
    config.thickness,
    config.halfLength,
  );
  syncObstacleFromSession();
  projection = computeProjectionFromPositions(livePositions, liveCapsule);
  updateLiveStatus();
  drawFrame();
}

function applyObstacleControls() {
  if (!liveSession || obstacleDragState) {
    return;
  }
  try {
    liveSession.setObstacle(
      obstacleKindControl.value,
      Number(obstacleXControl.value),
      Number(obstacleYControl.value),
      Number(obstacleZControl.value),
      Number(obstacleRadiusControl.value),
      Number(obstacleThicknessControl.value),
      Number(obstacleLengthControl.value) / 2,
    );
    syncObstacleFromSession();
    projection = computeProjectionFromPositions(livePositions, liveCapsule);
    updateLiveStatus();
    drawFrame();
  } catch (error) {
    status.textContent = `Could not change obstacle: ${formatError(error)}`;
  }
}

function obstacleBoundsProjection(positions, capsule) {
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
  if (liveObstacle?.kind === "sphere") {
    includeRadius(
      bounds,
      liveObstacle.center,
      liveObstacle.radius + liveObstacle.thickness,
    );
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

const baseComputeProjectionFromPositions = computeProjectionFromPositions;
computeProjectionFromPositions = function (positions, capsule = null) {
  if (!liveObstacle || liveObstacle.kind !== "sphere") {
    return baseComputeProjectionFromPositions(positions, capsule);
  }
  return obstacleBoundsProjection(positions, capsule);
};

const baseRefreshLiveTopology = refreshLiveTopology;
refreshLiveTopology = function () {
  baseRefreshLiveTopology();
  syncObstacleFromSession();
};

const baseActivateDemo = activateDemo;
activateDemo = async function (pins = null, autoplay = true) {
  const obstacle = captureObstacleConfig();
  await baseActivateDemo(pins, autoplay);
  restoreObstacleConfig(obstacle);
};

const baseActivateUpload = activateUpload;
activateUpload = async function (upload, pins = null) {
  const obstacle = captureObstacleConfig();
  await baseActivateUpload(upload, pins);
  restoreObstacleConfig(obstacle);
};

function drawSphereObstacle(obstacle) {
  const center = project(obstacle.center);
  const physicalRadius = projectedRadius(obstacle.center, obstacle.radius);
  const shellRadius = projectedRadius(
    obstacle.center,
    obstacle.radius + obstacle.thickness,
  );

  context.save();
  context.beginPath();
  context.arc(center.x, center.y, shellRadius, 0, Math.PI * 2);
  context.fillStyle = "rgba(240, 195, 109, 0.2)";
  context.fill();
  context.beginPath();
  context.arc(center.x, center.y, physicalRadius, 0, Math.PI * 2);
  context.fillStyle = "rgba(67, 88, 112, 0.96)";
  context.fill();
  context.beginPath();
  context.arc(center.x, center.y, physicalRadius, 0, Math.PI * 2);
  context.strokeStyle = "rgba(205, 220, 236, 0.48)";
  context.lineWidth = Math.max(1, canvas.width / 1400);
  context.stroke();
  context.restore();
}

function drawObstacleSelection() {
  if (!liveObstacle || interactionMode.value !== "obstacle") {
    return;
  }
  const center = project(liveObstacle.center);
  context.save();
  context.beginPath();
  context.arc(center.x, center.y, Math.max(7, canvas.width / 190), 0, Math.PI * 2);
  context.strokeStyle = "#f0c36d";
  context.lineWidth = Math.max(2, canvas.width / 900);
  context.stroke();
  context.restore();
}

const baseDrawFrame = drawFrame;
drawFrame = function () {
  if (liveSession && liveObstacle?.kind === "sphere" && projection) {
    resizeCanvas();
    context.clearRect(0, 0, canvas.width, canvas.height);
    drawSphereObstacle(liveObstacle);
    drawSurface(
      livePositions,
      liveTriangles,
      livePinned,
      dragState?.index ?? null,
      liveStretchEdges,
      liveBendingEdges,
    );
  } else {
    baseDrawFrame();
  }
  if (liveSession) {
    drawObstacleSelection();
  }
};

function pointSegmentDistance(point, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const denominator = dx * dx + dy * dy;
  if (denominator <= Number.EPSILON) {
    return Math.hypot(point.x - start.x, point.y - start.y);
  }
  const t = Math.min(
    1,
    Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / denominator),
  );
  const closestX = start.x + dx * t;
  const closestY = start.y + dy * t;
  return Math.hypot(point.x - closestX, point.y - closestY);
}

function obstacleHitTest(point) {
  if (!liveObstacle) {
    return false;
  }
  const padding = OBSTACLE_POINTER_PADDING * Math.min(window.devicePixelRatio || 1, 2);
  const shellRadius = projectedRadius(
    liveObstacle.center,
    liveObstacle.radius + liveObstacle.thickness,
  );
  if (liveObstacle.kind === "sphere") {
    const center = project(liveObstacle.center);
    return Math.hypot(point.x - center.x, point.y - center.y) <= shellRadius + padding;
  }
  const capsule = obstacleCapsuleDescriptor(liveObstacle);
  const start = project(capsule.start);
  const end = project(capsule.end);
  return pointSegmentDistance(point, start, end) <= shellRadius + padding;
}

function beginObstacleDrag(event) {
  if (interactionMode.value !== "obstacle") {
    return;
  }
  event.stopImmediatePropagation();
  if (!liveSession || obstacleDragState) {
    return;
  }
  if (!liveObstacle) {
    status.textContent = "Choose a sphere or capsule before moving the obstacle.";
    event.preventDefault();
    return;
  }
  const point = canvasPoint(event);
  if (!obstacleHitTest(point)) {
    status.textContent = "Start the drag on the collision obstacle.";
    event.preventDefault();
    return;
  }

  const projected = project(liveObstacle.center);
  obstacleDragState = {
    pointerId: event.pointerId,
    startX: point.x,
    startY: point.y,
    origin: [...liveObstacle.center],
    scale: Math.max(projected.scale, 1e-9),
  };
  canvas.setPointerCapture(event.pointerId);
  canvas.classList.add("dragging");
  event.preventDefault();
}

function moveObstacleDrag(event) {
  if (!obstacleDragState || event.pointerId !== obstacleDragState.pointerId) {
    return;
  }
  event.stopImmediatePropagation();
  const point = canvasPoint(event);
  const dx = (point.x - obstacleDragState.startX) / obstacleDragState.scale;
  const dy = (point.y - obstacleDragState.startY) / obstacleDragState.scale;
  const yawCos = Math.cos(YAW);
  const yawSin = Math.sin(YAW);
  const pitchCos = Math.cos(PITCH);
  const pitchSin = Math.sin(PITCH);
  const right = [yawCos, 0, -yawSin];
  const up = [-yawSin * pitchSin, pitchCos, -yawCos * pitchSin];
  const rawTarget = obstacleDragState.origin.map(
    (value, axis) => value + right[axis] * dx - up[axis] * dy,
  );
  const target = [
    clampToControl(obstacleXControl, rawTarget[0]),
    clampToControl(obstacleYControl, rawTarget[1]),
    clampToControl(obstacleZControl, rawTarget[2]),
  ];

  try {
    liveSession.setObstacle(
      liveObstacle.kind,
      target[0],
      target[1],
      target[2],
      liveObstacle.radius,
      liveObstacle.thickness,
      liveObstacle.halfLength,
    );
    syncObstacleFromSession();
    projection = computeProjectionFromPositions(livePositions, liveCapsule);
    updateLiveStatus();
    drawFrame();
  } catch (error) {
    status.textContent = `Obstacle drag stopped: ${formatError(error)}`;
    finishObstacleDrag(event);
  }
  event.preventDefault();
}

function finishObstacleDrag(event) {
  if (!obstacleDragState || event.pointerId !== obstacleDragState.pointerId) {
    return;
  }
  event.stopImmediatePropagation();
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
  obstacleDragState = null;
  canvas.classList.remove("dragging");
  event.preventDefault();
  drawFrame();
}

obstacleKindControl.addEventListener("change", () => {
  updateObstacleControlState();
  applyObstacleControls();
});

for (const control of [
  obstacleXControl,
  obstacleYControl,
  obstacleZControl,
  obstacleRadiusControl,
  obstacleThicknessControl,
  obstacleLengthControl,
]) {
  control.addEventListener("input", () => {
    updateObstacleLabels();
    applyObstacleControls();
  });
}

interactionMode.addEventListener("change", () => {
  canvas.classList.toggle("obstacle-tool", interactionMode.value === "obstacle");
  drawFrame();
});

canvas.addEventListener("pointerdown", beginObstacleDrag, true);
canvas.addEventListener("pointermove", moveObstacleDrag, true);
canvas.addEventListener("pointerup", finishObstacleDrag, true);
canvas.addEventListener("pointercancel", finishObstacleDrag, true);

updateObstacleControlState();
updateObstacleLabels();
updateObstacleInspector();
