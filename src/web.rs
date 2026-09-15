use wasm_bindgen::prelude::*;

use crate::{
    FixedStepConfig, GarmentAsset, GarmentImporter, GlbGarmentImporter, ObjGarmentImporter,
    ParticleDrag, TextilePreset, TriangleMeshCloth,
};

#[wasm_bindgen]
pub struct BrowserClothSession {
    initial: TriangleMeshCloth,
    cloth: TriangleMeshCloth,
    drag: Option<ParticleDrag>,
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

    pub fn step(&mut self) -> Result<(), JsValue> {
        self.cloth
            .step(FixedStepConfig::default())
            .map(|_| ())
            .map_err(js_error)
    }

    pub fn reset(&mut self) {
        self.cloth = self.initial.clone();
        self.drag = None;
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
            .update_particle_drag(drag, crate::Vec3::new(x, y, z))
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

    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.cloth.state_fingerprint().to_string()
    }
}

fn build_session(asset: GarmentAsset, preset: &str) -> Result<BrowserClothSession, JsValue> {
    let preset = parse_preset(preset)?;
    let config = preset.parameters().triangle_mesh_config(1.0);
    let cloth =
        TriangleMeshCloth::new(asset.positions(), asset.triangles(), config).map_err(js_error)?;
    Ok(BrowserClothSession {
        initial: cloth.clone(),
        cloth,
        drag: None,
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
