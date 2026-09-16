use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use crate::{
    CapsuleCollider, ClothCollider, ContactConfig, FixedStepConfig, GarmentAsset, GarmentImporter,
    GarmentSourceFormat, GlbGarmentImporter, ObjGarmentImporter, ParticleDrag, StepReport,
    TextilePreset, TriangleMeshCloth, Vec3,
};

const DEMO_MIN_RESOLUTION: u32 = 6;
const DEMO_MAX_RESOLUTION: u32 = 40;
const DEMO_WIDTH: f64 = 2.08;
const DEMO_ROWS_NUMERATOR: usize = 11;
const DEMO_ROWS_DENOMINATOR: usize = 13;
const MAX_BROWSER_SOLVER_ITERATIONS: u32 = 64;
const DEMO_CAPSULE: CapsuleCollider = CapsuleCollider {
    start: Vec3::new(0.52, -0.58, 0.88),
    end: Vec3::new(1.56, -0.58, 0.88),
    radius: 0.28,
    thickness: 0.025,
};

#[derive(Clone, Copy, Debug)]
struct BrowserPin {
    drag: ParticleDrag,
    target: Vec3,
}

#[wasm_bindgen]
pub struct BrowserClothSession {
    initial: TriangleMeshCloth,
    cloth: TriangleMeshCloth,
    drag: Option<ParticleDrag>,
    pins: BTreeMap<usize, BrowserPin>,
    step_config: FixedStepConfig,
    colliders: Vec<ClothCollider>,
    contact_config: ContactConfig,
    capsule: Option<CapsuleCollider>,
    source_kind: &'static str,
    normalized_asset_fingerprint: Option<u64>,
    last_report: Option<StepReport>,
}

#[wasm_bindgen]
impl BrowserClothSession {
    #[wasm_bindgen(js_name = fromObj)]
    pub fn from_obj(bytes: &[u8], preset: &str) -> Result<BrowserClothSession, JsValue> {
        let asset = ObjGarmentImporter.import(bytes).map_err(js_error)?;
        build_session(asset, preset)
    }

    #[wasm_bindgen(js_name = fromGlb)]
    pub fn from_glb(bytes: &[u8], preset: &str) -> Result<BrowserClothSession, JsValue> {
        let asset = GlbGarmentImporter.import(bytes).map_err(js_error)?;
        build_session(asset, preset)
    }

    #[wasm_bindgen(js_name = fromDemo)]
    pub fn from_demo(resolution: u32, preset: &str) -> Result<BrowserClothSession, JsValue> {
        if !(DEMO_MIN_RESOLUTION..=DEMO_MAX_RESOLUTION).contains(&resolution) {
            return Err(JsValue::from_str(
                "demo mesh resolution must be between 6 and 40",
            ));
        }

        let preset = parse_preset(preset)?;
        let columns = usize::try_from(resolution)
            .map_err(|_| JsValue::from_str("demo mesh resolution exceeds usize"))?;
        let rows = demo_rows(columns);
        let spacing = DEMO_WIDTH / (columns - 1) as f64;
        let mut positions = Vec::with_capacity(columns * rows);
        for row in 0..rows {
            for column in 0..columns {
                positions.push(Vec3::new(
                    column as f64 * spacing,
                    0.0,
                    row as f64 * spacing,
                ));
            }
        }

        let mut triangles = Vec::with_capacity((columns - 1) * (rows - 1) * 2);
        for row in 0..rows - 1 {
            for column in 0..columns - 1 {
                let top_left = row * columns + column;
                let top_right = top_left + 1;
                let bottom_left = (row + 1) * columns + column;
                let bottom_right = bottom_left + 1;
                triangles.push([top_left, bottom_left, top_right]);
                triangles.push([top_right, bottom_left, bottom_right]);
            }
        }

        let parameters = preset.parameters();
        let cloth =
            TriangleMeshCloth::new(&positions, &triangles, parameters.triangle_mesh_config(1.0))
                .map_err(js_error)?;
        let mut session = BrowserClothSession {
            initial: cloth.clone(),
            cloth,
            drag: None,
            pins: BTreeMap::new(),
            step_config: FixedStepConfig::default(),
            colliders: vec![ClothCollider::Capsule(DEMO_CAPSULE)],
            contact_config: parameters.contact_config(),
            capsule: Some(DEMO_CAPSULE),
            source_kind: "generated-sheet",
            normalized_asset_fingerprint: None,
            last_report: None,
        };
        session.pin_particle_at(0, positions[0])?;
        session.pin_particle_at(columns - 1, positions[columns - 1])?;
        Ok(session)
    }

    #[wasm_bindgen(js_name = vertexCount)]
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.cloth.particles().len()
    }

    #[wasm_bindgen(js_name = triangleCount)]
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.cloth.triangles().len()
    }

    #[wasm_bindgen(js_name = stretchConstraintCount)]
    #[must_use]
    pub fn stretch_constraint_count(&self) -> usize {
        self.cloth.stretch_constraints().len()
    }

    #[wasm_bindgen(js_name = bendingConstraintCount)]
    #[must_use]
    pub fn bending_constraint_count(&self) -> usize {
        self.cloth.bending_constraints().len()
    }

    #[must_use]
    pub fn positions(&self) -> Vec<f64> {
        self.cloth
            .particles()
            .iter()
            .flat_map(|particle| {
                let position = particle.position();
                [position.x, position.y, position.z]
            })
            .collect()
    }

    pub fn triangles(&self) -> Result<Vec<u32>, JsValue> {
        self.cloth
            .triangles()
            .iter()
            .flatten()
            .copied()
            .map(|index| {
                u32::try_from(index).map_err(|_| JsValue::from_str("triangle index exceeds u32"))
            })
            .collect()
    }

    #[wasm_bindgen(js_name = stretchConstraintEdges)]
    pub fn stretch_constraint_edges(&self) -> Result<Vec<u32>, JsValue> {
        let mut edges = Vec::with_capacity(self.cloth.stretch_constraints().len() * 2);
        for constraint in self.cloth.stretch_constraints() {
            edges.push(
                u32::try_from(constraint.particle_a)
                    .map_err(|_| JsValue::from_str("stretch constraint index exceeds u32"))?,
            );
            edges.push(
                u32::try_from(constraint.particle_b)
                    .map_err(|_| JsValue::from_str("stretch constraint index exceeds u32"))?,
            );
        }
        Ok(edges)
    }

    #[wasm_bindgen(js_name = bendingConstraintEdges)]
    pub fn bending_constraint_edges(&self) -> Result<Vec<u32>, JsValue> {
        let mut edges = Vec::with_capacity(self.cloth.bending_constraints().len() * 2);
        for constraint in self.cloth.bending_constraints() {
            edges.push(
                u32::try_from(constraint.edge_a)
                    .map_err(|_| JsValue::from_str("bending constraint index exceeds u32"))?,
            );
            edges.push(
                u32::try_from(constraint.edge_b)
                    .map_err(|_| JsValue::from_str("bending constraint index exceeds u32"))?,
            );
        }
        Ok(edges)
    }

    #[wasm_bindgen(js_name = pinnedIndices)]
    pub fn pinned_indices(&self) -> Result<Vec<u32>, JsValue> {
        self.pins
            .keys()
            .copied()
            .map(|index| {
                u32::try_from(index).map_err(|_| JsValue::from_str("particle index exceeds u32"))
            })
            .collect()
    }

    #[wasm_bindgen(js_name = capsuleCollider)]
    #[must_use]
    pub fn capsule_collider(&self) -> Vec<f64> {
        self.capsule.map_or_else(Vec::new, |capsule| {
            vec![
                capsule.start.x,
                capsule.start.y,
                capsule.start.z,
                capsule.end.x,
                capsule.end.y,
                capsule.end.z,
                capsule.radius,
                capsule.thickness,
            ]
        })
    }

    #[wasm_bindgen(js_name = sourceKind)]
    #[must_use]
    pub fn source_kind(&self) -> String {
        self.source_kind.to_owned()
    }

    #[wasm_bindgen(js_name = normalizedAssetFingerprint)]
    #[must_use]
    pub fn normalized_asset_fingerprint(&self) -> String {
        self.normalized_asset_fingerprint
            .map(|fingerprint| format!("{fingerprint:016x}"))
            .unwrap_or_default()
    }

    #[wasm_bindgen(js_name = maxStretchError)]
    #[must_use]
    pub fn max_stretch_error(&self) -> f64 {
        self.cloth.max_stretch_error()
    }

    #[wasm_bindgen(js_name = maxBendingError)]
    #[must_use]
    pub fn max_bending_error(&self) -> f64 {
        self.cloth.max_bending_error()
    }

    #[wasm_bindgen(js_name = lastCollisionProjections)]
    #[must_use]
    pub fn last_collision_projections(&self) -> usize {
        self.last_report
            .map_or(0, |report| report.collision_projections)
    }

    #[wasm_bindgen(js_name = lastFrictionCorrections)]
    #[must_use]
    pub fn last_friction_corrections(&self) -> usize {
        self.last_report
            .map_or(0, |report| report.friction_corrections)
    }

    pub fn step(&mut self) -> Result<(), JsValue> {
        let report = self
            .cloth
            .step_with_contacts(self.step_config, &self.colliders, self.contact_config)
            .map_err(js_error)?;
        self.last_report = Some(report);
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), JsValue> {
        let targets = self
            .pins
            .iter()
            .map(|(&index, pin)| (index, pin.target))
            .collect::<Vec<_>>();
        self.cloth = self.initial.clone();
        self.drag = None;
        self.pins.clear();
        self.last_report = None;
        for (index, target) in targets {
            self.pin_particle_at(index, target)?;
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = beginDrag)]
    pub fn begin_drag(&mut self, particle_index: u32) -> Result<(), JsValue> {
        if self.drag.is_some() {
            return Err(JsValue::from_str("a particle drag is already active"));
        }
        let particle_index = usize::try_from(particle_index)
            .map_err(|_| JsValue::from_str("particle index exceeds usize"))?;
        let drag = self
            .cloth
            .begin_particle_drag(particle_index)
            .map_err(js_error)?;
        self.drag = Some(drag);
        Ok(())
    }

    #[wasm_bindgen(js_name = dragTo)]
    pub fn drag_to(&mut self, x: f64, y: f64, z: f64) -> Result<(), JsValue> {
        let drag = self
            .drag
            .ok_or_else(|| JsValue::from_str("no particle drag is active"))?;
        self.cloth
            .update_particle_drag(drag, Vec3::new(x, y, z))
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = endDrag)]
    pub fn end_drag(&mut self) -> Result<(), JsValue> {
        let drag = self
            .drag
            .ok_or_else(|| JsValue::from_str("no particle drag is active"))?;
        self.cloth.end_particle_drag(drag).map_err(js_error)?;
        self.drag = None;
        Ok(())
    }

    #[wasm_bindgen(js_name = pinParticle)]
    pub fn pin_particle(&mut self, particle_index: u32) -> Result<(), JsValue> {
        let particle_index = usize::try_from(particle_index)
            .map_err(|_| JsValue::from_str("particle index exceeds usize"))?;
        if self.pins.contains_key(&particle_index) {
            return Ok(());
        }
        let target = self
            .cloth
            .particles()
            .get(particle_index)
            .ok_or_else(|| JsValue::from_str("particle index is outside the cloth"))?
            .position();
        self.pin_particle_at(particle_index, target)
    }

    #[wasm_bindgen(js_name = movePin)]
    pub fn move_pin(&mut self, particle_index: u32, x: f64, y: f64, z: f64) -> Result<(), JsValue> {
        let particle_index = usize::try_from(particle_index)
            .map_err(|_| JsValue::from_str("particle index exceeds usize"))?;
        let pin = self
            .pins
            .get(&particle_index)
            .copied()
            .ok_or_else(|| JsValue::from_str("particle is not pinned"))?;
        let target = Vec3::new(x, y, z);
        self.cloth
            .update_particle_drag(pin.drag, target)
            .map_err(js_error)?;
        if let Some(stored) = self.pins.get_mut(&particle_index) {
            stored.target = target;
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = unpinParticle)]
    pub fn unpin_particle(&mut self, particle_index: u32) -> Result<(), JsValue> {
        let particle_index = usize::try_from(particle_index)
            .map_err(|_| JsValue::from_str("particle index exceeds usize"))?;
        let pin = self
            .pins
            .get(&particle_index)
            .copied()
            .ok_or_else(|| JsValue::from_str("particle is not pinned"))?;
        self.cloth.end_particle_drag(pin.drag).map_err(js_error)?;
        self.pins.remove(&particle_index);
        Ok(())
    }

    #[wasm_bindgen(js_name = setGravity)]
    pub fn set_gravity(&mut self, gravity_y: f64) -> Result<(), JsValue> {
        if !gravity_y.is_finite() {
            return Err(JsValue::from_str("gravity must be finite"));
        }
        self.step_config.gravity = Vec3::new(0.0, gravity_y, 0.0);
        Ok(())
    }

    #[wasm_bindgen(js_name = setSolverIterations)]
    pub fn set_solver_iterations(&mut self, iterations: u32) -> Result<(), JsValue> {
        if iterations == 0 || iterations > MAX_BROWSER_SOLVER_ITERATIONS {
            return Err(JsValue::from_str(
                "solver iterations must be between 1 and 64",
            ));
        }
        self.step_config.solver_iterations = usize::try_from(iterations)
            .map_err(|_| JsValue::from_str("solver iteration count exceeds usize"))?;
        Ok(())
    }

    #[wasm_bindgen(js_name = setVelocityDamping)]
    pub fn set_velocity_damping(&mut self, damping: f64) -> Result<(), JsValue> {
        if !damping.is_finite() || !(0.0..=1.0).contains(&damping) {
            return Err(JsValue::from_str(
                "velocity damping must be between 0 and 1",
            ));
        }
        self.step_config.velocity_damping = damping;
        Ok(())
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.cloth.state_fingerprint().to_string()
    }
}

impl BrowserClothSession {
    fn pin_particle_at(&mut self, particle_index: usize, target: Vec3) -> Result<(), JsValue> {
        if self.drag.is_some() {
            return Err(JsValue::from_str(
                "finish the active particle drag before changing pins",
            ));
        }
        let drag = self
            .cloth
            .begin_particle_drag(particle_index)
            .map_err(js_error)?;
        if let Err(error) = self.cloth.update_particle_drag(drag, target) {
            let _ = self.cloth.end_particle_drag(drag);
            return Err(js_error(error));
        }
        self.pins
            .insert(particle_index, BrowserPin { drag, target });
        Ok(())
    }
}

fn build_session(asset: GarmentAsset, preset: &str) -> Result<BrowserClothSession, JsValue> {
    let preset = parse_preset(preset)?;
    let parameters = preset.parameters();
    let source_kind = match asset.source_format() {
        GarmentSourceFormat::Obj => "obj",
        GarmentSourceFormat::Glb => "glb",
    };
    let normalized_asset_fingerprint = asset.simulation_fingerprint();
    let cloth = TriangleMeshCloth::new(
        asset.positions(),
        asset.triangles(),
        parameters.triangle_mesh_config(1.0),
    )
    .map_err(js_error)?;
    Ok(BrowserClothSession {
        initial: cloth.clone(),
        cloth,
        drag: None,
        pins: BTreeMap::new(),
        step_config: FixedStepConfig::default(),
        colliders: Vec::new(),
        contact_config: parameters.contact_config(),
        capsule: None,
        source_kind,
        normalized_asset_fingerprint: Some(normalized_asset_fingerprint),
        last_report: None,
    })
}

fn demo_rows(columns: usize) -> usize {
    ((columns - 1) * DEMO_ROWS_NUMERATOR + DEMO_ROWS_DENOMINATOR / 2) / DEMO_ROWS_DENOMINATOR + 1
}

fn parse_preset(value: &str) -> Result<TextilePreset, JsValue> {
    match value {
        "cotton-like" => Ok(TextilePreset::CottonLike),
        "denim-like" => Ok(TextilePreset::DenimLike),
        "silk-like" => Ok(TextilePreset::SilkLike),
        "leather-like" => Ok(TextilePreset::LeatherLike),
        _ => Err(JsValue::from_str("unknown textile preset")),
    }
}

fn js_error(error: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
