"use strict";

const canvas = document.querySelector("#cloth-canvas");
const context = canvas.getContext("2d");
const playToggle = document.querySelector("#play-toggle");
const restartButton = document.querySelector("#restart");
const slider = document.querySelector("#frame-slider");
const status = document.querySelector("#status");

let snapshots = null;
let frameIndex = 0;
let playing = true;
let lastAdvance = 0;
let projection = null;

function includePoint(bounds, [x, y, z]) {
  bounds.minX = Math.min(bounds.minX, x);
  bounds.minY = Math.min(bounds.minY, y);
  bounds.minZ = Math.min(bounds.minZ, z);
  bounds.maxX = Math.max(bounds.maxX, x);
  bounds.maxY = Math.max(bounds.maxY, y);
  bounds.maxZ = Math.max(bounds.maxZ, z);
}

function computeProjection(data) {
  const bounds = {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
  };

  for (const frame of data.frames) {
    for (const position of frame.positions) {
      includePoint(bounds, position);
    }
  }

  if (data.sphere) {
    const radius = data.sphere.radius + data.sphere.thickness;
    const [x, y, z] = data.sphere.center;
    includePoint(bounds, [x - radius, y - radius, z - radius]);
    includePoint(bounds, [x + radius, y + radius, z + radius]);
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

  const yaw = -0.62;
  const pitch = 0.58;
  const yawCos = Math.cos(yaw);
  const yawSin = Math.sin(yaw);
  const pitchCos = Math.cos(pitch);
  const pitchSin = Math.sin(pitch);

  const rotatedX = yawCos * x - yawSin * z;
  const yawDepth = yawSin * x + yawCos * z;
  const rotatedY = pitchCos * y - pitchSin * yawDepth;
  const depth = pitchSin * y + pitchCos * yawDepth;

  const scale = Math.min(canvas.width, canvas.height) / (projection.span * 1.55);
  const perspective = 1 / Math.max(0.68, 1 + depth / (projection.span * 4.5));

  return {
    x: canvas.width / 2 + rotatedX * scale * perspective,
    y: canvas.height / 2 - rotatedY * scale * perspective,
    depth,
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

function drawSphere(sphere) {
  const center = project(sphere.center);
  const physicalRadius = projectedRadius(sphere.center, sphere.radius);
  const shellRadius = projectedRadius(sphere.center, sphere.radius + sphere.thickness);
  const gradient = context.createRadialGradient(
    center.x - physicalRadius * 0.28,
    center.y - physicalRadius * 0.32,
    physicalRadius * 0.12,
    center.x,
    center.y,
    physicalRadius,
  );
  gradient.addColorStop(0, "rgba(155, 177, 202, 0.78)");
  gradient.addColorStop(1, "rgba(46, 66, 88, 0.92)");

  context.beginPath();
  context.arc(center.x, center.y, physicalRadius, 0, Math.PI * 2);
  context.fillStyle = gradient;
  context.fill();
  context.strokeStyle = "rgba(206, 221, 236, 0.6)";
  context.lineWidth = Math.max(1, canvas.width / 1200);
  context.stroke();

  context.beginPath();
  context.arc(center.x, center.y, shellRadius, 0, Math.PI * 2);
  context.setLineDash([Math.max(4, canvas.width / 260), Math.max(4, canvas.width / 260)]);
  context.strokeStyle = "rgba(240, 195, 109, 0.72)";
  context.lineWidth = Math.max(1, canvas.width / 1400);
  context.stroke();
  context.setLineDash([]);
}

function drawFrame() {
  if (!snapshots) {
    return;
  }

  resizeCanvas();
  const frame = snapshots.frames[frameIndex];
  const projected = frame.positions.map(project);
  const triangles = snapshots.triangles
    .map((triangle) => ({
      triangle,
      depth:
        (projected[triangle[0]].depth +
          projected[triangle[1]].depth +
          projected[triangle[2]].depth) /
        3,
    }))
    .sort((a, b) => a.depth - b.depth);

  context.clearRect(0, 0, canvas.width, canvas.height);
  context.lineJoin = "round";

  if (snapshots.sphere) {
    drawSphere(snapshots.sphere);
  }

  for (const { triangle } of triangles) {
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

  for (const pinnedIndex of snapshots.pinned) {
    const point = projected[pinnedIndex];
    context.beginPath();
    context.arc(point.x, point.y, Math.max(3, canvas.width / 280), 0, Math.PI * 2);
    context.fillStyle = "#f0c36d";
    context.fill();
  }

  slider.value = String(frameIndex);
  status.textContent = `step ${frame.step} · fingerprint ${frame.fingerprint} · max stretch error ${frame.maxStretchError.toExponential(2)} · collision projections ${frame.collisionProjections}`;
}

function tick(timestamp) {
  if (snapshots && playing && timestamp - lastAdvance >= 1000 / 30) {
    frameIndex = (frameIndex + 1) % snapshots.frames.length;
    lastAdvance = timestamp;
    drawFrame();
  }
  window.requestAnimationFrame(tick);
}

function setPlaying(nextPlaying) {
  playing = nextPlaying;
  playToggle.textContent = playing ? "Pause" : "Play";
}

playToggle.addEventListener("click", () => {
  setPlaying(!playing);
});

restartButton.addEventListener("click", () => {
  frameIndex = 0;
  lastAdvance = 0;
  setPlaying(true);
  drawFrame();
});

slider.addEventListener("input", () => {
  frameIndex = Number(slider.value);
  setPlaying(false);
  drawFrame();
});

window.addEventListener("resize", drawFrame);

fetch("frames.json")
  .then((response) => {
    if (!response.ok) {
      throw new Error(`snapshot request failed with ${response.status}`);
    }
    return response.json();
  })
  .then((data) => {
    if (!Array.isArray(data.frames) || data.frames.length === 0) {
      throw new Error("snapshot payload has no frames");
    }
    if (!data.sphere || !Array.isArray(data.sphere.center)) {
      throw new Error("snapshot payload has no sphere collider metadata");
    }

    snapshots = data;
    projection = computeProjection(data);
    slider.max = String(data.frames.length - 1);
    slider.disabled = false;
    playToggle.disabled = false;
    restartButton.disabled = false;
    drawFrame();
    window.requestAnimationFrame(tick);
  })
  .catch((error) => {
    status.textContent = `Demo unavailable: ${error.message}`;
  });
