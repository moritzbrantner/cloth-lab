use core::fmt;
use std::collections::BTreeMap;

use crate::Vec3;

use collision::solve_collisions;
use fingerprint::{FNV_OFFSET_BASIS, hash_u64};
use geometry::cross;
use solver::{
    bending_constraint_value, max_constraint_error, solve_bending_constraint,
    solve_distance_constraint,
};
use topology::build_bending_constraints;
use validation::{validate_colliders, validate_contact_config, validate_step_config};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    position: Vec3,
    previous_position: Vec3,
    inverse_mass: f64,
}

impl Particle {
    #[must_use]
    pub const fn position(&self) -> Vec3 {
        self.position
    }

    #[must_use]
    pub const fn previous_position(&self) -> Vec3 {
        self.previous_position
    }

    #[must_use]
    pub const fn inverse_mass(&self) -> f64 {
        self.inverse_mass
    }

    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.inverse_mass <= f64::EPSILON
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceConstraint {
    pub particle_a: usize,
    pub particle_b: usize,
    pub rest_length: f64,
    pub compliance: f64,
    lambda: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BendingConstraint {
    pub opposite_a: usize,
    pub opposite_b: usize,
    pub edge_a: usize,
    pub edge_b: usize,
    pub compliance: f64,
    q: [[f64; 4]; 4],
    lambda: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangularClothConfig {
    pub columns: usize,
    pub rows: usize,
    pub spacing: f64,
    pub particle_mass: f64,
    pub stretch_compliance: f64,
    pub shear_compliance: f64,
    pub bending_compliance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedStepConfig {
    pub delta_seconds: f64,
    pub gravity: Vec3,
    pub solver_iterations: usize,
    pub velocity_damping: f64,
}

impl Default for FixedStepConfig {
    fn default() -> Self {
        Self {
            delta_seconds: 1.0 / 60.0,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            solver_iterations: 12,
            velocity_damping: 0.995,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactConfig {
    pub friction_coefficient: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereCollider {
    pub center: Vec3,
    pub radius: f64,
    pub thickness: f64,
}

impl SphereCollider {
    #[must_use]
    pub fn effective_radius(self) -> f64 {
        self.radius + self.thickness
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapsuleCollider {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f64,
    pub thickness: f64,
}

impl CapsuleCollider {
    #[must_use]
    pub fn effective_radius(self) -> f64 {
        self.radius + self.thickness
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClothCollider {
    Sphere(SphereCollider),
    Capsule(CapsuleCollider),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepReport {
    pub max_stretch_error: f64,
    pub max_shear_error: f64,
    pub max_bending_error: f64,
    pub state_fingerprint: u64,
    pub collision_projections: usize,
    pub friction_corrections: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cloth {
    columns: usize,
    rows: usize,
    particles: Vec<Particle>,
    triangles: Vec<[usize; 3]>,
    stretch_constraints: Vec<DistanceConstraint>,
    shear_constraints: Vec<DistanceConstraint>,
    bending_constraints: Vec<BendingConstraint>,
}

impl Cloth {
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    #[must_use]
    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    #[must_use]
    pub fn stretch_constraints(&self) -> &[DistanceConstraint] {
        &self.stretch_constraints
    }

    #[must_use]
    pub fn shear_constraints(&self) -> &[DistanceConstraint] {
        &self.shear_constraints
    }

    #[must_use]
    pub fn bending_constraints(&self) -> &[BendingConstraint] {
        &self.bending_constraints
    }

    pub fn pin(&mut self, particle_index: usize) -> Result<(), ClothError> {
        let particle = self
            .particles
            .get_mut(particle_index)
            .ok_or(ClothError::InvalidParticleIndex)?;
        particle.inverse_mass = 0.0;
        particle.previous_position = particle.position;
        Ok(())
    }

    pub fn pin_top_corners(&mut self) -> Result<(), ClothError> {
        self.pin(0)?;
        self.pin(self.columns - 1)
    }

    pub fn step(&mut self, config: FixedStepConfig) -> Result<StepReport, ClothError> {
        self.step_with_colliders(config, &[])
    }

    pub fn step_with_colliders(
        &mut self,
        config: FixedStepConfig,
        colliders: &[ClothCollider],
    ) -> Result<StepReport, ClothError> {
        self.step_with_contacts(config, colliders, ContactConfig::default())
    }

    pub fn step_with_contacts(
        &mut self,
        config: FixedStepConfig,
        colliders: &[ClothCollider],
        contact: ContactConfig,
    ) -> Result<StepReport, ClothError> {
        validate_step_config(config)?;
        validate_contact_config(contact)?;
        validate_colliders(colliders)?;

        let delta_squared = config.delta_seconds * config.delta_seconds;
        for particle in &mut self.particles {
            if particle.is_pinned() {
                particle.previous_position = particle.position;
                continue;
            }

            let current_position = particle.position;
            let inertial_displacement =
                (particle.position - particle.previous_position) * config.velocity_damping;
            particle.position += inertial_displacement + config.gravity * delta_squared;
            particle.previous_position = current_position;
        }

        for constraint in &mut self.stretch_constraints {
            constraint.lambda = 0.0;
        }
        for constraint in &mut self.shear_constraints {
            constraint.lambda = 0.0;
        }
        for constraint in &mut self.bending_constraints {
            constraint.lambda = 0.0;
        }

        let mut collision_projections = 0;
        let mut friction_corrections = 0;
        for _ in 0..config.solver_iterations {
            let particles = &mut self.particles;
            for constraint in &mut self.stretch_constraints {
                solve_distance_constraint(particles, constraint, config.delta_seconds);
            }
            for constraint in &mut self.shear_constraints {
                solve_distance_constraint(particles, constraint, config.delta_seconds);
            }
            for constraint in &mut self.bending_constraints {
                solve_bending_constraint(particles, constraint, config.delta_seconds);
            }
            let collision_report = solve_collisions(particles, colliders, contact);
            collision_projections += collision_report.projections;
            friction_corrections += collision_report.friction_corrections;
        }

        Ok(StepReport {
            max_stretch_error: self.max_stretch_error(),
            max_shear_error: self.max_shear_error(),
            max_bending_error: self.max_bending_error(),
            state_fingerprint: self.state_fingerprint(),
            collision_projections,
            friction_corrections,
        })
    }

    #[must_use]
    pub fn max_stretch_error(&self) -> f64 {
        max_constraint_error(&self.particles, &self.stretch_constraints)
    }

    #[must_use]
    pub fn max_shear_error(&self) -> f64 {
        max_constraint_error(&self.particles, &self.shear_constraints)
    }

    #[must_use]
    pub fn max_bending_error(&self) -> f64 {
        self.bending_constraints
            .iter()
            .map(|constraint| bending_constraint_value(&self.particles, constraint).abs())
            .fold(0.0, f64::max)
    }

    #[must_use]
    pub fn state_fingerprint(&self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        hash_u64(&mut hash, self.columns as u64);
        hash_u64(&mut hash, self.rows as u64);
        for particle in &self.particles {
            for value in [
                particle.position.x,
                particle.position.y,
                particle.position.z,
                particle.previous_position.x,
                particle.previous_position.y,
                particle.previous_position.z,
                particle.inverse_mass,
            ] {
                hash_u64(&mut hash, value.to_bits());
            }
        }
        hash
    }
}
