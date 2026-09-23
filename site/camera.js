"use strict";

const cameraYawControl = document.querySelector("#camera-yaw");
const cameraPitchControl = document.querySelector("#camera-pitch");
const cameraZoomControl = document.querySelector("#camera-zoom");
const cameraPanXControl = document.querySelector("#camera-pan-x");
const cameraPanYControl = document.querySelector("#camera-pan-y");
const cameraYawValue = document.querySelector("#camera-yaw-value");
const cameraPitchValue = document.querySelector("#camera-pitch-value");
const cameraZoomValue = document.querySelector("#camera-zoom-value");
const cameraPanXValue = document.querySelector("#camera-pan-x-value");
const cameraPanYValue = document.querySelector("#camera-pan-y-value");
const resetCameraButton = document.querySelector("#reset-camera");

const DEFAULT_CAMERA_YAW = -0.62;
const DEFAULT_CAMERA_PITCH = 0.58;
const MIN_CAMERA_PITCH = (-80 * Math.PI) / 180;
const MAX_CAMERA_PITCH = (80 * Math.PI) / 180;

let cameraYaw = DEFAULT_CAMERA_YAW;
let cameraPitch = DEFAULT_CAMERA_PITCH;
let cameraZoom = 1;
let cameraPanX = 0;
let cameraPanY = 0;
let cameraDragState = null;

function degrees(radiansValue) {
  return (radiansValue * 180) / Math.PI;
}

function radians(degreesValue) {
  return (degreesValue * Math.PI) / 180;
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}

function normalizeYaw(value) {
  const fullTurn = Math.PI * 2;
  return ((((value + Math.PI) % fullTurn) + fullTurn) % fullTurn) - Math.PI;
}

function cameraBasis() {
  const yawCos = Math.cos(cameraYaw);
  const yawSin = Math.sin(cameraYaw);
  const pitchCos = Math.cos(cameraPitch);
  const pitchSin = Math.sin(cameraPitch);
  return {
    right: [yawCos, 0, -yawSin],
    up: [-yawSin * pitchSin, pitchCos, -yawCos * pitchSin],
  };
}

function updateCameraLabels() {
  cameraYawValue.textContent = `${Math.round(degrees(cameraYaw))}°`;
  cameraPitchValue.textContent = `${Math.round(degrees(cameraPitch))}°`;
  cameraZoomValue.textContent = `${cameraZoom.toFixed(2)}×`;
  cameraPanXValue.textContent = cameraPanX.toFixed(2);
  cameraPanYValue.textContent = cameraPanY.toFixed(2);
}

function syncCameraControls() {
  cameraYawControl.value = String(degrees(cameraYaw));
  cameraPitchControl.value = String(degrees(cameraPitch));
  cameraZoomControl.value = String(cameraZoom);
  cameraPanXControl.value = String(cameraPanX);
  cameraPanYControl.value = String(cameraPanY);
  updateCameraLabels();
}

project = function (position) {
  const [centerX, centerY, centerZ] = projection.center;
  const x = position[0] - centerX;
  const y = position[1] - centerY;
  const z = position[2] - centerZ;

  const yawCos = Math.cos(cameraYaw);
  const yawSin = Math.sin(cameraYaw);
  const pitchCos = Math.cos(cameraPitch);
  const pitchSin = Math.sin(cameraPitch);

  const rotatedX = yawCos * x - yawSin * z;
  const yawDepth = yawSin * x + yawCos * z;
  const rotatedY = pitchCos * y - pitchSin * yawDepth;
  const depth = pitchSin * y + pitchCos * yawDepth;

  const baseScale =
    (Math.min(canvas.width, canvas.height) / (projection.span * 1.55)) * cameraZoom;
  const perspective = 1 / Math.max(0.68, 1 + depth / (projection.span * 4.5));

  return {
    x:
      canvas.width / 2 +
      rotatedX * baseScale * perspective +
      cameraPanX * canvas.width,
    y:
      canvas.height / 2 -
      rotatedY * baseScale * perspective +
      cameraPanY * canvas.height,
    depth,
    scale: baseScale * perspective,
  };
};

pointerTarget = function (event) {
  const point = canvasPoint(event);
  const dx = (point.x - dragState.startX) / dragState.scale;
  const dy = (point.y - dragState.startY) / dragState.scale;
  const { right, up } = cameraBasis();
  return dragState.origin.map((value, axis) => value + right[axis] * dx - up[axis] * dy);
};

function moveObstacleDragCameraAware(event) {
  if (!obstacleDragState || event.pointerId !== obstacleDragState.pointerId) {
    return;
  }
  event.stopImmediatePropagation();
  const point = canvasPoint(event);
  const dx = (point.x - obstacleDragState.startX) / obstacleDragState.scale;
  const dy = (point.y - obstacleDragState.startY) / obstacleDragState.scale;
  const { right, up } = cameraBasis();
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
    updateLiveStatus();
    drawFrame();
  } catch (error) {
    status.textContent = `Obstacle drag stopped: ${formatError(error)}`;
    finishObstacleDrag(event);
  }
  event.preventDefault();
}

canvas.removeEventListener("pointermove", moveObstacleDrag, true);
canvas.addEventListener("pointermove", moveObstacleDragCameraAware, true);

function beginCameraDrag(event) {
  if (interactionMode.value !== "camera") {
    return;
  }
  event.stopImmediatePropagation();
  if (cameraDragState) {
    return;
  }
  cameraDragState = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    yaw: cameraYaw,
    pitch: cameraPitch,
  };
  canvas.setPointerCapture(event.pointerId);
  canvas.classList.add("dragging");
  event.preventDefault();
}

function updateCameraDrag(event) {
  if (!cameraDragState || event.pointerId !== cameraDragState.pointerId) {
    return;
  }
  event.stopImmediatePropagation();
  cameraYaw = normalizeYaw(
    cameraDragState.yaw + (event.clientX - cameraDragState.startX) * 0.008,
  );
  cameraPitch = clamp(
    cameraDragState.pitch - (event.clientY - cameraDragState.startY) * 0.008,
    MIN_CAMERA_PITCH,
    MAX_CAMERA_PITCH,
  );
  syncCameraControls();
  drawFrame();
  event.preventDefault();
}

function finishCameraDrag(event) {
  if (!cameraDragState || event.pointerId !== cameraDragState.pointerId) {
    return;
  }
  event.stopImmediatePropagation();
  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
  cameraDragState = null;
  canvas.classList.remove("dragging");
  event.preventDefault();
}

function applyCameraControls() {
  cameraYaw = radians(Number(cameraYawControl.value));
  cameraPitch = clamp(
    radians(Number(cameraPitchControl.value)),
    MIN_CAMERA_PITCH,
    MAX_CAMERA_PITCH,
  );
  cameraZoom = clamp(Number(cameraZoomControl.value), 0.5, 3);
  cameraPanX = clamp(Number(cameraPanXControl.value), -0.5, 0.5);
  cameraPanY = clamp(Number(cameraPanYControl.value), -0.5, 0.5);
  // Preserve exact degree/zoom/pan entry; only external camera changes resync fields.
  updateCameraLabels();
  drawFrame();
}

for (const control of [
  cameraYawControl,
  cameraPitchControl,
  cameraZoomControl,
  cameraPanXControl,
  cameraPanYControl,
]) {
  control.addEventListener("input", applyCameraControls);
}

resetCameraButton.addEventListener("click", () => {
  cameraYaw = DEFAULT_CAMERA_YAW;
  cameraPitch = DEFAULT_CAMERA_PITCH;
  cameraZoom = 1;
  cameraPanX = 0;
  cameraPanY = 0;
  syncCameraControls();
  drawFrame();
});

interactionMode.addEventListener("change", () => {
  canvas.classList.toggle("camera-tool", interactionMode.value === "camera");
});

canvas.addEventListener("pointerdown", beginCameraDrag, true);
canvas.addEventListener("pointermove", updateCameraDrag, true);
canvas.addEventListener("pointerup", finishCameraDrag, true);
canvas.addEventListener("pointercancel", finishCameraDrag, true);

syncCameraControls();
