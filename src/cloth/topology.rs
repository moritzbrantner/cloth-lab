use std::collections::BTreeMap;

use super::geometry::{cross, dot};
use super::validation::validate_rectangular_config;
use super::{
    BendingConstraint, Cloth, ClothError, DistanceConstraint, Particle, RectangularClothConfig,
};
use crate::Vec3;

#[derive(Clone, Copy, Debug)]
struct PendingBendingEdge {
    start: usize,
    end: usize,
    opposite: usize,
}

impl Cloth {
    pub fn rectangular(config: RectangularClothConfig) -> Result<Self, ClothError> {
        validate_rectangular_config(config)?;

        let particle_count = config
            .columns
            .checked_mul(config.rows)
            .ok_or(ClothError::TopologyTooLarge)?;
        let quad_count = (config.columns - 1)
            .checked_mul(config.rows - 1)
            .ok_or(ClothError::TopologyTooLarge)?;
        let triangle_count = quad_count
            .checked_mul(2)
            .ok_or(ClothError::TopologyTooLarge)?;
        let horizontal_constraint_count = (config.columns - 1)
            .checked_mul(config.rows)
            .ok_or(ClothError::TopologyTooLarge)?;
        let vertical_constraint_count = (config.rows - 1)
            .checked_mul(config.columns)
            .ok_or(ClothError::TopologyTooLarge)?;
        let stretch_constraint_count = horizontal_constraint_count
            .checked_add(vertical_constraint_count)
            .ok_or(ClothError::TopologyTooLarge)?;
        let shear_constraint_count = quad_count
            .checked_mul(2)
            .ok_or(ClothError::TopologyTooLarge)?;

        let inverse_mass = 1.0 / config.particle_mass;
        let mut particles = Vec::with_capacity(particle_count);
        for row in 0..config.rows {
            for column in 0..config.columns {
                let position = Vec3::new(
                    column as f64 * config.spacing,
                    0.0,
                    row as f64 * config.spacing,
                );
                particles.push(Particle {
                    position,
                    previous_position: position,
                    inverse_mass,
                });
            }
        }

        let mut triangles = Vec::with_capacity(triangle_count);
        for row in 0..(config.rows - 1) {
            for column in 0..(config.columns - 1) {
                let top_left = row * config.columns + column;
                let top_right = top_left + 1;
                let bottom_left = (row + 1) * config.columns + column;
                let bottom_right = bottom_left + 1;
                triangles.push([top_left, bottom_left, top_right]);
                triangles.push([top_right, bottom_left, bottom_right]);
            }
        }

        let mut stretch_constraints = Vec::with_capacity(stretch_constraint_count);
        for row in 0..config.rows {
            for column in 0..(config.columns - 1) {
                let particle_a = row * config.columns + column;
                stretch_constraints.push(DistanceConstraint {
                    particle_a,
                    particle_b: particle_a + 1,
                    rest_length: config.spacing,
                    compliance: config.stretch_compliance,
                    lambda: 0.0,
                });
            }
        }
        for row in 0..(config.rows - 1) {
            for column in 0..config.columns {
                let particle_a = row * config.columns + column;
                stretch_constraints.push(DistanceConstraint {
                    particle_a,
                    particle_b: particle_a + config.columns,
                    rest_length: config.spacing,
                    compliance: config.stretch_compliance,
                    lambda: 0.0,
                });
            }
        }

        let shear_rest_length = config.spacing * std::f64::consts::SQRT_2;
        let mut shear_constraints = Vec::with_capacity(shear_constraint_count);
        for row in 0..(config.rows - 1) {
            for column in 0..(config.columns - 1) {
                let top_left = row * config.columns + column;
                let top_right = top_left + 1;
                let bottom_left = (row + 1) * config.columns + column;
                let bottom_right = bottom_left + 1;
                shear_constraints.push(DistanceConstraint {
                    particle_a: top_left,
                    particle_b: bottom_right,
                    rest_length: shear_rest_length,
                    compliance: config.shear_compliance,
                    lambda: 0.0,
                });
                shear_constraints.push(DistanceConstraint {
                    particle_a: top_right,
                    particle_b: bottom_left,
                    rest_length: shear_rest_length,
                    compliance: config.shear_compliance,
                    lambda: 0.0,
                });
            }
        }

        let bending_constraints =
            build_bending_constraints(&particles, &triangles, config.bending_compliance)?;

        Ok(Self {
            columns: config.columns,
            rows: config.rows,
            particles,
            triangles,
            stretch_constraints,
            shear_constraints,
            bending_constraints,
        })
    }
}

pub(super) fn build_bending_constraints(
    particles: &[Particle],
    triangles: &[[usize; 3]],
    compliance: f64,
) -> Result<Vec<BendingConstraint>, ClothError> {
    let mut pending = BTreeMap::<(usize, usize), PendingBendingEdge>::new();
    let mut constraints = Vec::new();

    for triangle in triangles {
        let [a, b, c] = *triangle;
        for (start, end, opposite) in [(a, b, c), (b, c, a), (c, a, b)] {
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            if let Some(first) = pending.remove(&key) {
                debug_assert_eq!((start, end), (first.end, first.start));
                let q = init_isometric_bending_matrix(
                    particles[first.opposite].position,
                    particles[opposite].position,
                    particles[first.start].position,
                    particles[first.end].position,
                )
                .ok_or(ClothError::DegenerateBendingStencil)?;
                constraints.push(BendingConstraint {
                    opposite_a: first.opposite,
                    opposite_b: opposite,
                    edge_a: first.start,
                    edge_b: first.end,
                    compliance,
                    q,
                    lambda: 0.0,
                });
            } else {
                pending.insert(
                    key,
                    PendingBendingEdge {
                        start,
                        end,
                        opposite,
                    },
                );
            }
        }
    }
    Ok(constraints)
}

fn init_isometric_bending_matrix(
    opposite_a: Vec3,
    opposite_b: Vec3,
    edge_a: Vec3,
    edge_b: Vec3,
) -> Option<[[f64; 4]; 4]> {
    let x = [edge_a, edge_b, opposite_a, opposite_b];
    let e0 = x[1] - x[0];
    let e1 = x[2] - x[0];
    let e2 = x[3] - x[0];
    let e3 = x[2] - x[1];
    let e4 = x[3] - x[1];

    let c01 = cot_theta(e0, e1)?;
    let c02 = cot_theta(e0, e2)?;
    let c03 = cot_theta(e0 * -1.0, e3)?;
    let c04 = cot_theta(e0 * -1.0, e4)?;

    let area_a = 0.5 * cross(e0, e1).length();
    let area_b = 0.5 * cross(e0, e2).length();
    let area_sum = area_a + area_b;
    if !area_sum.is_finite() || area_sum <= f64::EPSILON {
        return None;
    }

    let coefficient = -3.0 / (2.0 * area_sum);
    let k = [c03 + c04, c01 + c02, -c01 - c03, -c02 - c04];
    let mut q = [[0.0; 4]; 4];
    for (row, q_row) in q.iter_mut().enumerate() {
        for (column, value) in q_row.iter_mut().enumerate() {
            *value = k[row] * coefficient * k[column];
            if !value.is_finite() {
                return None;
            }
        }
    }
    Some(q)
}

fn cot_theta(left: Vec3, right: Vec3) -> Option<f64> {
    let cross_length = cross(left, right).length();
    if !cross_length.is_finite() || cross_length <= f64::EPSILON {
        return None;
    }
    let cotangent = dot(left, right) / cross_length;
    cotangent.is_finite().then_some(cotangent)
}
