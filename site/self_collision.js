"use strict";

const selfCollisionMode = document.querySelector("#self-collision-mode");
const selfCollisionThickness = document.querySelector("#self-collision-thickness");
const selfCollisionThicknessValue = document.querySelector("#self-collision-thickness-value");
const inspectSelfCollision = document.querySelector("#inspect-self-collision");

function selfCollisionEnabled() {
  return selfCollisionMode.value === "on";
}

function updateSelfCollisionControls() {
  selfCollisionThickness.disabled = !selfCollisionEnabled();
  selfCollisionThicknessValue.textContent = Number(selfCollisionThickness.value).toFixed(3);
}

function applySelfCollisionControl(session) {
  session.setSelfCollision(
    selfCollisionEnabled(),
    Number(selfCollisionThickness.value),
  );
}

const baseApplyRuntimeControlsForSelfCollision = applyRuntimeControls;
applyRuntimeControls = function (session) {
  baseApplyRuntimeControlsForSelfCollision(session);
  applySelfCollisionControl(session);
};

const baseUpdateLiveInspectorForSelfCollision = updateLiveInspector;
updateLiveInspector = function () {
  baseUpdateLiveInspectorForSelfCollision();
  if (!liveSession || !inspectSelfCollision) {
    return;
  }
  if (!selfCollisionEnabled()) {
    inspectSelfCollision.textContent = "off";
    return;
  }
  inspectSelfCollision.textContent = `${liveSession.lastSelfCollisionCandidates()} candidates · ${liveSession.lastSelfCollisionTests()} tests · ${liveSession.lastSelfCollisionProjections()} projections`;
};

function applySelfCollisionToLiveSession() {
  updateSelfCollisionControls();
  if (!liveSession) {
    return;
  }
  try {
    applySelfCollisionControl(liveSession);
    updateLiveStatus();
  } catch (error) {
    status.textContent = `Could not change self collision: ${formatError(error)}`;
  }
}

selfCollisionMode.addEventListener("change", applySelfCollisionToLiveSession);
selfCollisionThickness.addEventListener("input", applySelfCollisionToLiveSession);

updateSelfCollisionControls();
if (inspectSelfCollision) {
  inspectSelfCollision.textContent = "off";
}
