use core::fmt;

use crate::Vec3;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClothError {
    DimensionsTooSmall,
    InvalidSpacing,
    InvalidParticleMass,
    InvalidCompliance,
    TopologyTooLarge,
    InvalidParticleIndex,
    InvalidTimeStep,
    InvalidGravity,
    InvalidSolverIterations,
    InvalidVelocityDamping,
    InvalidColliderCenter,
    InvalidColliderRadius,
    InvalidCollisionThickness,
}

impl fmt::Display for ClothError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DimensionsTooSmall => "cloth dimensions must be at least 2 by 2",
            Self::InvalidSpacing => "cloth spacing must be finite and positive",
            Self::InvalidParticleMass => "particle mass must be finite and positive",
            Self::InvalidCompliance => "constraint compliance must be finite and non-negative",
            Self::TopologyTooLarge => "cloth topology dimensions overflow addressable size",
            Self::InvalidParticleIndex => "particle index is outside the cloth",
            Self::InvalidTimeStep => "fixed timestep must be finite and positive",
            Self::InvalidGravity => "gravity must contain only finite components",
            Self::InvalidSolverIterations => "solver iteration count must be positive",
            Self::InvalidVelocityDamping => "velocity damping must be finite and between 0 and 1",
            Self::InvalidColliderCenter => "collider center must contain only finite components",
            Self::InvalidColliderRadius => "collider radius must be finite and positive",
            Self::InvalidCollisionThickness => {
                "collision thickness must be finite, non-negative, and produce a finite shell"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ClothError {}

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
pub struct RectangularClothConfig {
    pub columns: usize,
    pub rows: usize,
    pub spacing: f64,
    pub particle_mass: f64,
    pub stretch_compliance: f64,
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
pub enum ClothCollider {
    Sphere(SphereCollider),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepReport {
    pub max_stretch_error: f64,
    pub state_fingerprint: u64,
    pub collision_projections: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cloth {
    columns: usize,
    rows: usize,
    particles: Vec<Particle>,
    triangles: Vec<[usize; 3]>,
    stretch_constraints: Vec<DistanceConstraint>,
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
        let constraint_count = horizontal_constraint_count
            .checked_add(vertical_constraint_count)
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

        let mut stretch_constraints = Vec::with_capacity(constraint_count);
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

        Ok(Self {
            columns: config.columns,
            rows: config.rows,
            particles,
            triangles,
            stretch_constraints,
        })
    }

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
        validate_step_config(config)?;
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

        let mut collision_projections = 0;
        for _ in 0..config.solver_iterations {
            let particles = &mut self.particles;
            for constraint in &mut self.stretch_constraints {
                solve_distance_constraint(particles, constraint, config.delta_seconds);
            }
            collision_projections += solve_collisions(particles, colliders);
        }

        Ok(StepReport {
            max_stretch_error: self.max_stretch_error(),
            state_fingerprint: self.state_fingerprint(),
            collision_projections,
        })
    }

    #[must_use]
    pub fn max_stretch_error(&self) -> f64 {
        self.stretch_constraints
            .iter()
            .map(|constraint| {
                let delta = self.particles[constraint.particle_a].position
                    - self.particles[constraint.particle_b].position;
                (delta.length() - constraint.rest_length).abs()
            })
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

fn validate_rectangular_config(config: RectangularClothConfig) -> Result<(), ClothError> {
    if config.columns < 2 || config.rows < 2 {
        return Err(ClothError::DimensionsTooSmall);
    }
    if !config.spacing.is_finite() || config.spacing <= 0.0 {
        return Err(ClothError::InvalidSpacing);
    }
    if !config.particle_mass.is_finite() || config.particle_mass <= 0.0 {
        return Err(ClothError::InvalidParticleMass);
    }
    if !config.stretch_compliance.is_finite() || config.stretch_compliance < 0.0 {
        return Err(ClothError::InvalidCompliance);
    }
    Ok(())
}

fn validate_step_config(config: FixedStepConfig) -> Result<(), ClothError> {
    if !config.delta_seconds.is_finite() || config.delta_seconds <= 0.0 {
        return Err(ClothError::InvalidTimeStep);
    }
    if !config.gravity.is_finite() {
        return Err(ClothError::InvalidGravity);
    }
    if config.solver_iterations == 0 {
        return Err(ClothError::InvalidSolverIterations);
    }
    if !config.velocity_damping.is_finite() || !(0.0..=1.0).contains(&config.velocity_damping) {
        return Err(ClothError::InvalidVelocityDamping);
    }
    Ok(())
}

fn validate_colliders(colliders: &[ClothCollider]) -> Result<(), ClothError> {
    for collider in colliders {
        match collider {
            ClothCollider::Sphere(sphere) => validate_sphere_collider(*sphere)?,
        }
    }
    Ok(())
}

fn validate_sphere_collider(collider: SphereCollider) -> Result<(), ClothError> {
    if !collider.center.is_finite() {
        return Err(ClothError::InvalidColliderCenter);
    }
    if !collider.radius.is_finite() || collider.radius <= 0.0 {
        return Err(ClothError::InvalidColliderRadius);
    }
    if !collider.thickness.is_finite()
        || collider.thickness < 0.0
        || !collider.effective_radius().is_finite()
    {
        return Err(ClothError::InvalidCollisionThickness);
    }
    Ok(())
}

fn solve_distance_constraint(
    particles: &mut [Particle],
    constraint: &mut DistanceConstraint,
    delta_seconds: f64,
) {
    let (particle_a, particle_b) = two_mut(particles, constraint.particle_a, constraint.particle_b);
    let delta = particle_a.position - particle_b.position;
    let length = delta.length();
    if length <= f64::EPSILON {
        return;
    }

    let weight_sum = particle_a.inverse_mass + particle_b.inverse_mass;
    if weight_sum <= f64::EPSILON {
        return;
    }

    let alpha = constraint.compliance / (delta_seconds * delta_seconds);
    let constraint_error = length - constraint.rest_length;
    let delta_lambda = (-constraint_error - alpha * constraint.lambda) / (weight_sum + alpha);
    constraint.lambda += delta_lambda;

    let normal = delta / length;
    particle_a.position += normal * (particle_a.inverse_mass * delta_lambda);
    particle_b.position -= normal * (particle_b.inverse_mass * delta_lambda);
}

fn solve_collisions(particles: &mut [Particle], colliders: &[ClothCollider]) -> usize {
    let mut projections = 0;
    for particle in particles {
        if particle.is_pinned() {
            continue;
        }
        for collider in colliders {
            let projected = match collider {
                ClothCollider::Sphere(sphere) => solve_sphere_collision(particle, *sphere),
            };
            projections += usize::from(projected);
        }
    }
    projections
}

fn solve_sphere_collision(particle: &mut Particle, collider: SphereCollider) -> bool {
    let effective_radius = collider.effective_radius();
    let current_delta = particle.position - collider.center;
    let current_distance = current_delta.length();

    if current_distance < effective_radius {
        let previous_delta = particle.previous_position - collider.center;
        let normal = collision_normal(current_delta, previous_delta);
        particle.position = collider.center + normal * effective_radius;
        return true;
    }

    let movement = particle.position - particle.previous_position;
    let movement_squared = movement.length_squared();
    if movement_squared <= f64::EPSILON {
        return false;
    }

    let start_delta = particle.previous_position - collider.center;
    let radius_squared = effective_radius * effective_radius;
    let start_distance_squared = start_delta.length_squared();
    if start_distance_squared <= radius_squared {
        return false;
    }

    let projection = start_delta.x * movement.x
        + start_delta.y * movement.y
        + start_delta.z * movement.z;
    let constant = start_distance_squared - radius_squared;
    let discriminant = projection * projection - movement_squared * constant;
    if discriminant < 0.0 {
        return false;
    }

    let hit_fraction = (-projection - discriminant.sqrt()) / movement_squared;
    if !(0.0..=1.0).contains(&hit_fraction) {
        return false;
    }

    let hit = particle.previous_position + movement * hit_fraction;
    let hit_delta = hit - collider.center;
    let normal = collision_normal(hit_delta, start_delta);
    particle.position = collider.center + normal * effective_radius;
    true
}

fn collision_normal(primary: Vec3, fallback: Vec3) -> Vec3 {
    let primary_length = primary.length();
    if primary_length > f64::EPSILON {
        return primary / primary_length;
    }

    let fallback_length = fallback.length();
    if fallback_length > f64::EPSILON {
        return fallback / fallback_length;
    }

    Vec3::new(0.0, 1.0, 0.0)
}

fn two_mut<T>(slice: &mut [T], first: usize, second: usize) -> (&mut T, &mut T) {
    debug_assert_ne!(first, second);
    if first < second {
        let (left, right) = slice.split_at_mut(second);
        (&mut left[first], &mut right[0])
    } else {
        let (left, right) = slice.split_at_mut(first);
        (&mut right[0], &mut left[second])
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloth_fixture() -> Cloth {
        let mut cloth = Cloth::rectangular(RectangularClothConfig {
            columns: 6,
            rows: 6,
            spacing: 0.2,
            particle_mass: 1.0,
            stretch_compliance: 1.0e-7,
        })
        .expect("fixture cloth must be valid");
        cloth
            .pin_top_corners()
            .expect("fixture corner indices must be valid");
        cloth
    }

    fn sphere_fixture() -> ClothCollider {
        ClothCollider::Sphere(SphereCollider {
            center: Vec3::new(0.5, -0.5, 0.5),
            radius: 0.35,
            thickness: 0.02,
        })
    }

    #[test]
    fn rectangular_topology_has_stable_ordering() {
        let cloth = cloth_fixture();

        assert_eq!(cloth.particles().len(), 36);
        assert_eq!(cloth.triangles().len(), 50);
        assert_eq!(cloth.stretch_constraints().len(), 60);
        assert_eq!(cloth.triangles()[0], [0, 6, 1]);
        assert_eq!(cloth.triangles()[49], [29, 34, 35]);
    }

    #[test]
    fn pinned_corners_remain_exactly_fixed() {
        let mut cloth = cloth_fixture();
        let left = cloth.particles()[0].position();
        let right = cloth.particles()[cloth.columns() - 1].position();

        for _ in 0..240 {
            cloth
                .step(FixedStepConfig::default())
                .expect("fixed step must remain valid");
        }

        assert_eq!(cloth.particles()[0].position(), left);
        assert_eq!(cloth.particles()[cloth.columns() - 1].position(), right);
    }

    #[test]
    fn repeated_runs_produce_identical_state() {
        let mut first = cloth_fixture();
        let mut second = cloth_fixture();

        for _ in 0..240 {
            first
                .step(FixedStepConfig::default())
                .expect("first deterministic step must succeed");
            second
                .step(FixedStepConfig::default())
                .expect("second deterministic step must succeed");
        }

        assert_eq!(first.particles(), second.particles());
        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
    }

    #[test]
    fn hanging_sheet_falls_and_keeps_stretch_bounded() {
        let mut cloth = cloth_fixture();
        let bottom_middle = (cloth.rows() - 1) * cloth.columns() + cloth.columns() / 2;
        let initial_height = cloth.particles()[bottom_middle].position().y;

        for _ in 0..360 {
            cloth
                .step(FixedStepConfig::default())
                .expect("fixed step must remain valid");
        }

        assert!(cloth.particles()[bottom_middle].position().y < initial_height - 0.1);
        assert!(cloth.max_stretch_error() < 0.02);
    }

    #[test]
    fn sphere_collision_preserves_shell_and_replay() {
        let collider = sphere_fixture();
        let mut first = cloth_fixture();
        let mut second = cloth_fixture();
        let mut observed_collision = false;

        for _ in 0..240 {
            let first_report = first
                .step_with_colliders(FixedStepConfig::default(), &[collider])
                .expect("sphere collision step must remain valid");
            let second_report = second
                .step_with_colliders(FixedStepConfig::default(), &[collider])
                .expect("replayed sphere collision step must remain valid");
            assert_eq!(first_report, second_report);
            observed_collision |= first_report.collision_projections > 0;
        }

        assert!(observed_collision);
        assert_eq!(first.particles(), second.particles());

        let ClothCollider::Sphere(sphere) = collider;
        let minimum_distance = sphere.effective_radius() - 1.0e-9;
        for particle in first.particles().iter().filter(|particle| !particle.is_pinned()) {
            assert!((particle.position() - sphere.center).length() >= minimum_distance);
        }
    }

    #[test]
    fn swept_sphere_collision_blocks_tunneling() {
        let collider = SphereCollider {
            center: Vec3::ZERO,
            radius: 0.25,
            thickness: 0.05,
        };
        let mut particle = Particle {
            position: Vec3::new(1.0, 0.0, 0.0),
            previous_position: Vec3::new(-1.0, 0.0, 0.0),
            inverse_mass: 1.0,
        };

        assert!(solve_sphere_collision(&mut particle, collider));
        assert!((particle.position.x + collider.effective_radius()).abs() < 1.0e-12);
        assert!(particle.position.y.abs() < 1.0e-12);
        assert!(particle.position.z.abs() < 1.0e-12);
    }

    #[test]
    fn invalid_collider_fails_closed_before_advancing() {
        let mut cloth = cloth_fixture();
        let fingerprint = cloth.state_fingerprint();
        let invalid = ClothCollider::Sphere(SphereCollider {
            center: Vec3::ZERO,
            radius: 0.5,
            thickness: -0.1,
        });

        let error = cloth
            .step_with_colliders(FixedStepConfig::default(), &[invalid])
            .expect_err("negative collision thickness must be rejected");

        assert_eq!(error, ClothError::InvalidCollisionThickness);
        assert_eq!(cloth.state_fingerprint(), fingerprint);
    }

    #[test]
    fn invalid_step_configuration_fails_closed() {
        let mut cloth = cloth_fixture();
        let error = cloth
            .step(FixedStepConfig {
                delta_seconds: 0.0,
                ..FixedStepConfig::default()
            })
            .expect_err("zero timestep must be rejected");

        assert_eq!(error, ClothError::InvalidTimeStep);
    }
}
