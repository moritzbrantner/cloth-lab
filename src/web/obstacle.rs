use wasm_bindgen::prelude::*;

use super::{BrowserClothSession, js_error};
use crate::{ClothCollider, ClothObstacleConfig, ClothObstacleKind, Vec3};

#[wasm_bindgen]
impl BrowserClothSession {
    #[wasm_bindgen(js_name = obstacleDescriptor)]
    #[must_use]
    pub fn obstacle_descriptor(&self) -> Vec<f64> {
        let Some(collider) = self.colliders.first() else {
            return vec![0.0];
        };

        match collider {
            ClothCollider::Sphere(sphere) => vec![
                1.0,
                sphere.center.x,
                sphere.center.y,
                sphere.center.z,
                sphere.radius,
                sphere.thickness,
                0.0,
            ],
            ClothCollider::Capsule(capsule) => {
                let center = (capsule.start + capsule.end) / 2.0;
                let half_length = (capsule.end - capsule.start).length() / 2.0;
                vec![
                    2.0,
                    center.x,
                    center.y,
                    center.z,
                    capsule.radius,
                    capsule.thickness,
                    half_length,
                ]
            }
        }
    }

    #[wasm_bindgen(js_name = setObstacle)]
    pub fn set_obstacle(
        &mut self,
        kind: &str,
        center_x: f64,
        center_y: f64,
        center_z: f64,
        radius: f64,
        thickness: f64,
        capsule_half_length: f64,
    ) -> Result<(), JsValue> {
        if self.drag.is_some() {
            return Err(JsValue::from_str(
                "finish the active particle drag before changing the obstacle",
            ));
        }

        let kind = match kind {
            "none" => ClothObstacleKind::None,
            "sphere" => ClothObstacleKind::Sphere,
            "capsule" => ClothObstacleKind::Capsule,
            _ => return Err(JsValue::from_str("unknown cloth obstacle kind")),
        };
        let config = ClothObstacleConfig {
            kind,
            center: Vec3::new(center_x, center_y, center_z),
            radius,
            thickness,
            capsule_half_length,
        };
        let colliders = config.colliders().map_err(js_error)?;
        let capsule = colliders.first().and_then(|collider| match collider {
            ClothCollider::Capsule(capsule) => Some(*capsule),
            ClothCollider::Sphere(_) => None,
        });

        self.colliders = colliders;
        self.capsule = capsule;
        self.last_report = None;
        Ok(())
    }
}
