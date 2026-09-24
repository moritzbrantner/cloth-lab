use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use crate::{
    CapsuleCollider, ClothCollider, ContactConfig, FixedStepConfig, GarmentAsset, GarmentImporter,
    GarmentSourceFormat, GarmentTemplate, GarmentTemplateAsset, GlbGarmentImporter,
    MannequinAnimation, ObjGarmentImporter, ParticleDrag, SelfCollisionConfig, SelfCollisionReport,
    StepReport, TextilePreset, TriangleMeshCloth, Vec3,
    mannequin::{MannequinAnimator, MannequinAttachment},
};

mod obstacle;

const MAX_BROWSER_SOLVER_ITERATIONS: u32 = 64;

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
    fixed_collider_count: usize,
    mannequin: Option<MannequinAnimator>,
    mannequin_attachments: BTreeMap<usize, MannequinAttachment>,
    contact_config: ContactConfig,
    capsule: Option<CapsuleCollider>,
    source_kind: &'static str,
    normalized_asset_fingerprint: Option<u64>,
    self_collision_config: Option<SelfCollisionConfig>,
    last_report: Option<StepReport>,
    last_self_collision_report: SelfCollisionReport,
}

#[wasm_bindgen(js_name = mannequinAnimationCatalog)]
pub fn mannequin_animation_catalog() -> Vec<String> {
    let mut catalog = Vec::with_capacity(MannequinAnimation::ALL.len() * 2);
    for animation in MannequinAnimation::ALL {
        catalog.push(animation.key().to_owned());
        catalog.push(animation.display_name().to_owned());
    }
    catalog
}

#[wasm_bindgen(js_name = garmentTemplateCatalog)]
pub fn garment_template_catalog() -> Vec<String> {
    let mut catalog = Vec::with_capacity(GarmentTemplate::ALL.len() * 3);
    for template in GarmentTemplate::ALL {
        catalog.push(template.key().to_owned());
        catalog.push(template.display_name().to_owned());
        catalog.push(template.summary().to_owned());
    }
    catalog
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
        Self::from_template("sheet", resolution, preset)
    }

    #[wasm_bindgen(js_name = fromTemplate)]
    pub fn from_template(
        template: &str,
        resolution: u32,
        preset: &str,
    ) -> Result<BrowserClothSession, JsValue> {
        let template = GarmentTemplate::from_key(template)
            .ok_or_else(|| JsValue::from_str("unknown garment template"))?;
        let asset = template.build(resolution).map_err(js_error)?;
        build_template_session(asset, preset)
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

    #[wasm_bindgen(js_name = sceneColliders)]
    #[must_use]
    pub fn scene_colliders(&self) -> Vec<f64> {
        let mut descriptors = Vec::with_capacity(self.fixed_collider_count * 9);
        for collider in &self.colliders[..self.fixed_collider_count] {
            match collider {
                ClothCollider::Sphere(sphere) => descriptors.extend_from_slice(&[
                    1.0,
                    sphere.center.x,
                    sphere.center.y,
                    sphere.center.z,
                    sphere.center.x,
                    sphere.center.y,
                    sphere.center.z,
                    sphere.radius,
                    sphere.thickness,
                ]),
                ClothCollider::Capsule(capsule) => descriptors.extend_from_slice(&[
                    2.0,
                    capsule.start.x,
                    capsule.start.y,
                    capsule.start.z,
                    capsule.end.x,
                    capsule.end.y,
                    capsule.end.z,
                    capsule.radius,
                    capsule.thickness,
                ]),
            }
        }
        descriptors
    }

    #[wasm_bindgen(js_name = hasMannequin)]
    #[must_use]
    pub fn has_mannequin(&self) -> bool {
        self.mannequin.is_some()
    }

    #[wasm_bindgen(js_name = mannequinJointCount)]
    #[must_use]
    pub fn mannequin_joint_count(&self) -> usize {
        self.mannequin
            .as_ref()
            .map_or(0, MannequinAnimator::joint_count)
    }

    #[wasm_bindgen(js_name = mannequinAnimation)]
    #[must_use]
    pub fn mannequin_animation(&self) -> String {
        self.mannequin
            .as_ref()
            .map(|mannequin| mannequin.animation().key().to_owned())
            .unwrap_or_default()
    }

    #[wasm_bindgen(js_name = mannequinAnimationTime)]
    #[must_use]
    pub fn mannequin_animation_time(&self) -> f64 {
        self.mannequin
            .as_ref()
            .map_or(0.0, |mannequin| f64::from(mannequin.time_seconds()))
    }

    #[wasm_bindgen(js_name = mannequinAnimationSpeed)]
    #[must_use]
    pub fn mannequin_animation_speed(&self) -> f64 {
        self.mannequin
            .as_ref()
            .map_or(0.0, |mannequin| f64::from(mannequin.speed()))
    }

    #[wasm_bindgen(js_name = setMannequinAnimation)]
    pub fn set_mannequin_animation(&mut self, value: &str) -> Result<(), JsValue> {
        let animation = MannequinAnimation::from_key(value)
            .ok_or_else(|| JsValue::from_str("unknown mannequin animation"))?;
        let mannequin = self
            .mannequin
            .as_mut()
            .ok_or_else(|| JsValue::from_str("this cloth session has no mannequin"))?;
        mannequin.set_animation(animation);
        self.sync_mannequin_state()
    }

    #[wasm_bindgen(js_name = setMannequinAnimationSpeed)]
    pub fn set_mannequin_animation_speed(&mut self, speed: f64) -> Result<(), JsValue> {
        let speed = speed as f32;
        let mannequin = self
            .mannequin
            .as_mut()
            .ok_or_else(|| JsValue::from_str("this cloth session has no mannequin"))?;
        mannequin.set_speed(speed).map_err(js_error)
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

    #[wasm_bindgen(js_name = lastSelfCollisionCandidates)]
    #[must_use]
    pub fn last_self_collision_candidates(&self) -> usize {
        self.last_self_collision_report.vertex_triangle_candidates
    }

    #[wasm_bindgen(js_name = lastSelfCollisionTests)]
    #[must_use]
    pub fn last_self_collision_tests(&self) -> usize {
        self.last_self_collision_report.narrow_phase_tests
    }

    #[wasm_bindgen(js_name = lastSelfCollisionProjections)]
    #[must_use]
    pub fn last_self_collision_projections(&self) -> usize {
        self.last_self_collision_report.projections
    }

    #[wasm_bindgen(js_name = setSelfCollision)]
    pub fn set_self_collision(&mut self, enabled: bool, thickness: f64) -> Result<(), JsValue> {
        let next = if enabled {
            let config = SelfCollisionConfig { thickness };
            config.validate().map_err(js_error)?;
            Some(config)
        } else {
            None
        };
        self.self_collision_config = next;
        self.last_self_collision_report = SelfCollisionReport::default();
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), JsValue> {
        if let Some(mannequin) = self.mannequin.as_mut() {
            mannequin
                .advance(self.step_config.delta_seconds)
                .map_err(js_error)?;
            self.sync_mannequin_state()?;
        }

        if let Some(self_collision) = self.self_collision_config {
            let report = self
                .cloth
                .step_with_contacts_and_self_collision(
                    self.step_config,
                    &self.colliders,
                    self.contact_config,
                    self_collision,
                )
                .map_err(js_error)?;
            self.last_report = Some(report.solver);
            self.last_self_collision_report = report.self_collision;
        } else {
            let report = self
                .cloth
                .step_with_contacts(self.step_config, &self.colliders, self.contact_config)
                .map_err(js_error)?;
            self.last_report = Some(report);
            self.last_self_collision_report = SelfCollisionReport::default();
        }
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
        self.last_self_collision_report = SelfCollisionReport::default();
        for (index, target) in targets {
            self.pin_particle_at(index, target)?;
        }
        if let Some(mannequin) = self.mannequin.as_mut() {
            mannequin.reset();
            self.sync_mannequin_state()?;
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
        self.mannequin_attachments.remove(&particle_index);
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
        self.mannequin_attachments.remove(&particle_index);
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

    fn sync_mannequin_state(&mut self) -> Result<(), JsValue> {
        let Some(mannequin) = self.mannequin.as_ref() else {
            return Ok(());
        };
        if mannequin.colliders().len() != self.fixed_collider_count {
            return Err(JsValue::from_str(
                "mannequin collider topology changed unexpectedly",
            ));
        }

        for (target, source) in self.colliders[..self.fixed_collider_count]
            .iter_mut()
            .zip(mannequin.colliders())
        {
            *target = *source;
        }

        let attachment_targets = self
            .mannequin_attachments
            .values()
            .copied()
            .map(|attachment| {
                (
                    attachment.particle_index,
                    mannequin.attachment_target(attachment),
                )
            })
            .collect::<Vec<_>>();
        for (particle_index, target) in attachment_targets {
            let pin = self
                .pins
                .get(&particle_index)
                .copied()
                .ok_or_else(|| JsValue::from_str("mannequin attachment lost its cloth pin"))?;
            self.cloth
                .update_particle_drag(pin.drag, target)
                .map_err(js_error)?;
            if let Some(stored) = self.pins.get_mut(&particle_index) {
                stored.target = target;
            }
        }
        Ok(())
    }
}

fn build_template_session(
    asset: GarmentTemplateAsset,
    preset: &str,
) -> Result<BrowserClothSession, JsValue> {
    let preset = parse_preset(preset)?;
    let parameters = preset.parameters();
    let positions = asset.positions().to_vec();
    let triangles = asset.triangles().to_vec();
    let pinned_indices = asset.pinned_indices().to_vec();
    let mannequin_attachments = asset
        .mannequin_attachments()
        .iter()
        .copied()
        .map(|attachment| (attachment.particle_index, attachment))
        .collect::<BTreeMap<_, _>>();
    let source_kind = asset.template().source_kind();
    let normalized_asset_fingerprint = asset.simulation_fingerprint();
    let mannequin = asset.template().uses_mannequin().then(MannequinAnimator::new);
    let mut colliders = mannequin
        .as_ref()
        .map_or_else(Vec::new, |mannequin| mannequin.colliders().to_vec());
    let fixed_collider_count = colliders.len();
    let editable_obstacle = asset.editable_obstacle();
    if let Some(collider) = editable_obstacle {
        colliders.push(collider);
    }
    let capsule = editable_obstacle.and_then(|collider| match collider {
        ClothCollider::Capsule(capsule) => Some(capsule),
        ClothCollider::Sphere(_) => None,
    });
    let cloth =
        TriangleMeshCloth::new(&positions, &triangles, parameters.triangle_mesh_config(1.0))
            .map_err(js_error)?;
    let mut session = BrowserClothSession {
        initial: cloth.clone(),
        cloth,
        drag: None,
        pins: BTreeMap::new(),
        step_config: FixedStepConfig::default(),
        colliders,
        fixed_collider_count,
        mannequin,
        mannequin_attachments,
        contact_config: parameters.contact_config(),
        capsule,
        source_kind,
        normalized_asset_fingerprint: Some(normalized_asset_fingerprint),
        self_collision_config: None,
        last_report: None,
        last_self_collision_report: SelfCollisionReport::default(),
    };
    for index in pinned_indices {
        let target = positions
            .get(index)
            .copied()
            .ok_or_else(|| JsValue::from_str("template pin index is outside the cloth"))?;
        session.pin_particle_at(index, target)?;
    }
    session.sync_mannequin_state()?;
    Ok(session)
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
        fixed_collider_count: 0,
        mannequin: None,
        mannequin_attachments: BTreeMap::new(),
        contact_config: parameters.contact_config(),
        capsule: None,
        source_kind,
        normalized_asset_fingerprint: Some(normalized_asset_fingerprint),
        self_collision_config: None,
        last_report: None,
        last_self_collision_report: SelfCollisionReport::default(),
    })
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
