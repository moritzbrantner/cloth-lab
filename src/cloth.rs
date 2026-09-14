use core::fmt;
use std::collections::BTreeMap;

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
    InvalidColliderAxis,
    InvalidColliderRadius,
    InvalidCollisionThickness,
    DegenerateBendingStencil,
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
            Self::InvalidColliderAxis => {
                "capsule endpoints must be finite and define a non-degenerate axis"
            }
            Self::InvalidColliderRadius => "collider radius must be finite and positive",
            Self::InvalidCollisionThickness => {
                "collision thickness must be finite, non-negative, and produce a finite shell"
            }
            Self::DegenerateBendingStencil => {
                "cloth bending stencil must be finite and contain two non-degenerate triangles"
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

        let bending_constraints = build_bending_constraints(
            &particles,
            &triangles,
            config.bending_compliance,
        )?;

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
        for constraint in &mut self.shear_constraints {
            constraint.lambda = 0.0;
        }
        for constraint in &mut self.bending_constraints {
            constraint.lambda = 0.0;
        }

        let mut collision_projections = 0;
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
            collision_projections += solve_collisions(particles, colliders);
        }

        Ok(StepReport {
            max_stretch_error: self.max_stretch_error(),
            max_shear_error: self.max_shear_error(),
            max_bending_error: self.max_bending_error(),
            state_fingerprint: self.state_fingerprint(),
            collision_projections,
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

fn build_bending_constraints(
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
    let k = [
        c03 + c04,
        c01 + c02,
        -c01 - c03,
        -c02 - c04,
    ];
    let mut q = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            q[row][column] = k[row] * coefficient * k[column];
            if !q[row][column].is_finite() {
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

fn max_constraint_error(particles: &[Particle], constraints: &[DistanceConstraint]) -> f64 {
    constraints
        .iter()
        .map(|constraint| {
            let delta = particles[constraint.particle_a].position
                - particles[constraint.particle_b].position;
            (delta.length() - constraint.rest_length).abs()
        })
        .fold(0.0, f64::max)
}

fn bending_constraint_value(particles: &[Particle], constraint: &BendingConstraint) -> f64 {
    let indices = [
        constraint.edge_a,
        constraint.edge_b,
        constraint.opposite_a,
        constraint.opposite_b,
    ];
    let positions = indices.map(|index| particles[index].position);
    bending_energy(&positions, &constraint.q)
}

fn bending_energy(positions: &[Vec3; 4], q: &[[f64; 4]; 4]) -> f64 {
    let mut energy = 0.0;
    for row in 0..4 {
        for column in 0..4 {
            energy += q[row][column] * dot(positions[column], positions[row]);
        }
    }
    energy * 0.5
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
    if [
        config.stretch_compliance,
        config.shear_compliance,
        config.bending_compliance,
    ]
    .into_iter()
    .any(|compliance| !compliance.is_finite() || compliance < 0.0)
    {
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
            ClothCollider::Capsule(capsule) => validate_capsule_collider(*capsule)?,
        }
    }
    Ok(())
}

fn validate_sphere_collider(collider: SphereCollider) -> Result<(), ClothError> {
    if !collider.center.is_finite() {
        return Err(ClothError::InvalidColliderCenter);
    }
    validate_radius_and_thickness(collider.radius, collider.thickness)
}

fn validate_capsule_collider(collider: CapsuleCollider) -> Result<(), ClothError> {
    let axis = collider.end - collider.start;
    if !collider.start.is_finite()
        || !collider.end.is_finite()
        || axis.length_squared() <= f64::EPSILON
    {
        return Err(ClothError::InvalidColliderAxis);
    }
    validate_radius_and_thickness(collider.radius, collider.thickness)
}

fn validate_radius_and_thickness(radius: f64, thickness: f64) -> Result<(), ClothError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ClothError::InvalidColliderRadius);
    }
    if !thickness.is_finite() || thickness < 0.0 || !(radius + thickness).is_finite() {
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

fn solve_bending_constraint(
    particles: &mut [Particle],
    constraint: &mut BendingConstraint,
    delta_seconds: f64,
) {
    let indices = [
        constraint.edge_a,
        constraint.edge_b,
        constraint.opposite_a,
        constraint.opposite_b,
    ];
    let positions = indices.map(|index| particles[index].position);
    let inverse_masses = indices.map(|index| particles[index].inverse_mass);
    let energy = bending_energy(&positions, &constraint.q);

    let mut gradients = [Vec3::ZERO; 4];
    for row in 0..4 {
        for column in 0..4 {
            gradients[row] += positions[column] * constraint.q[row][column];
        }
    }

    let alpha = constraint.compliance / (delta_seconds * delta_seconds);
    let mut denominator = alpha;
    for index in 0..4 {
        denominator += inverse_masses[index] * gradients[index].length_squared();
    }
    if !denominator.is_finite() || denominator <= f64::EPSILON {
        return;
    }

    let delta_lambda = -(energy + alpha * constraint.lambda) / denominator;
    if !delta_lambda.is_finite() {
        return;
    }
    constraint.lambda += delta_lambda;

    for index in 0..4 {
        let correction = gradients[index] * (inverse_masses[index] * delta_lambda);
        particles[indices[index]].position += correction;
    }
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
                ClothCollider::Capsule(capsule) => solve_capsule_collision(particle, *capsule),
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

    let projection = dot(start_delta, movement);
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

fn solve_capsule_collision(particle: &mut Particle, collider: CapsuleCollider) -> bool {
    let effective_radius = collider.effective_radius();
    let radius_squared = effective_radius * effective_radius;
    let axis = collider.end - collider.start;
    let current_axis_point =
        closest_point_on_segment(particle.position, collider.start, collider.end);
    let current_delta = particle.position - current_axis_point;

    if current_delta.length_squared() < radius_squared {
        let previous_axis_point =
            closest_point_on_segment(particle.previous_position, collider.start, collider.end);
        let previous_delta = particle.previous_position - previous_axis_point;
        let normal = capsule_collision_normal(current_delta, previous_delta, axis);
        particle.position = current_axis_point + normal * effective_radius;
        return true;
    }

    let movement = particle.position - particle.previous_position;
    if movement.length_squared() <= f64::EPSILON {
        return false;
    }

    let start_axis_point =
        closest_point_on_segment(particle.previous_position, collider.start, collider.end);
    let start_delta = particle.previous_position - start_axis_point;
    if start_delta.length_squared() <= radius_squared {
        return false;
    }

    let (closest_fraction, closest_distance_squared) = closest_path_fraction_to_segment(
        particle.previous_position,
        particle.position,
        collider.start,
        collider.end,
    );
    if closest_distance_squared > radius_squared || closest_fraction <= f64::EPSILON {
        return false;
    }

    let mut outside_fraction = 0.0;
    let mut inside_fraction = closest_fraction;
    for _ in 0..48 {
        let candidate_fraction = (outside_fraction + inside_fraction) * 0.5;
        let candidate = particle.previous_position + movement * candidate_fraction;
        let candidate_axis_point =
            closest_point_on_segment(candidate, collider.start, collider.end);
        let distance_squared = (candidate - candidate_axis_point).length_squared();
        if distance_squared > radius_squared {
            outside_fraction = candidate_fraction;
        } else {
            inside_fraction = candidate_fraction;
        }
    }

    let hit = particle.previous_position + movement * inside_fraction;
    let hit_axis_point = closest_point_on_segment(hit, collider.start, collider.end);
    let hit_delta = hit - hit_axis_point;
    let normal = capsule_collision_normal(hit_delta, start_delta, axis);
    particle.position = hit_axis_point + normal * effective_radius;
    true
}

fn closest_point_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let segment = end - start;
    let segment_length_squared = segment.length_squared();
    debug_assert!(segment_length_squared > f64::EPSILON);
    let fraction = (dot(point - start, segment) / segment_length_squared).clamp(0.0, 1.0);
    start + segment * fraction
}

fn closest_path_fraction_to_segment(
    path_start: Vec3,
    path_end: Vec3,
    segment_start: Vec3,
    segment_end: Vec3,
) -> (f64, f64) {
    let path = path_end - path_start;
    let segment = segment_end - segment_start;
    let offset = path_start - segment_start;
    let path_length_squared = dot(path, path);
    let segment_projection = dot(path, segment);
    let segment_length_squared = dot(segment, segment);
    let path_offset_projection = dot(path, offset);
    let segment_offset_projection = dot(segment, offset);
    let denominator =
        path_length_squared * segment_length_squared - segment_projection * segment_projection;

    let mut path_numerator;
    let mut path_denominator = denominator;
    let mut segment_numerator;
    let mut segment_denominator = denominator;

    if denominator <= f64::EPSILON {
        path_numerator = 0.0;
        path_denominator = 1.0;
        segment_numerator = segment_offset_projection;
        segment_denominator = segment_length_squared;
    } else {
        path_numerator = segment_projection * segment_offset_projection
            - segment_length_squared * path_offset_projection;
        segment_numerator = path_length_squared * segment_offset_projection
            - segment_projection * path_offset_projection;

        if path_numerator < 0.0 {
            path_numerator = 0.0;
            segment_numerator = segment_offset_projection;
            segment_denominator = segment_length_squared;
        } else if path_numerator > path_denominator {
            path_numerator = path_denominator;
            segment_numerator = segment_offset_projection + segment_projection;
            segment_denominator = segment_length_squared;
        }
    }

    if segment_numerator < 0.0 {
        segment_numerator = 0.0;
        if -path_offset_projection < 0.0 {
            path_numerator = 0.0;
        } else if -path_offset_projection > path_length_squared {
            path_numerator = path_denominator;
        } else {
            path_numerator = -path_offset_projection;
            path_denominator = path_length_squared;
        }
    } else if segment_numerator > segment_denominator {
        segment_numerator = segment_denominator;
        let end_projection = -path_offset_projection + segment_projection;
        if end_projection < 0.0 {
            path_numerator = 0.0;
        } else if end_projection > path_length_squared {
            path_numerator = path_denominator;
        } else {
            path_numerator = end_projection;
            path_denominator = path_length_squared;
        }
    }

    let path_fraction = if path_numerator.abs() <= f64::EPSILON {
        0.0
    } else {
        path_numerator / path_denominator
    };
    let segment_fraction = if segment_numerator.abs() <= f64::EPSILON {
        0.0
    } else {
        segment_numerator / segment_denominator
    };
    let separation = offset + path * path_fraction - segment * segment_fraction;
    (path_fraction, separation.length_squared())
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

fn capsule_collision_normal(primary: Vec3, fallback: Vec3, axis: Vec3) -> Vec3 {
    let primary_length = primary.length();
    if primary_length > f64::EPSILON {
        return primary / primary_length;
    }

    let fallback_length = fallback.length();
    if fallback_length > f64::EPSILON {
        return fallback / fallback_length;
    }

    deterministic_perpendicular(axis)
}

fn deterministic_perpendicular(axis: Vec3) -> Vec3 {
    let absolute_x = axis.x.abs();
    let absolute_y = axis.y.abs();
    let absolute_z = axis.z.abs();
    let perpendicular = if absolute_x <= absolute_y && absolute_x <= absolute_z {
        Vec3::new(0.0, -axis.z, axis.y)
    } else if absolute_y <= absolute_z {
        Vec3::new(-axis.z, 0.0, axis.x)
    } else {
        Vec3::new(-axis.y, axis.x, 0.0)
    };
    perpendicular / perpendicular.length()
}

fn cross(left: Vec3, right: Vec3) -> Vec3 {
    Vec3::new(
        left.y * right.z - left.z * right.y,
        left.z * right.x - left.x * right.z,
        left.x * right.y - left.y * right.x,
    )
}

fn dot(left: Vec3, right: Vec3) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
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
            shear_compliance: 2.5e-7,
            bending_compliance: 1.0e-3,
        })
        .expect("fixture cloth must be valid");
        cloth
            .pin_top_corners()
            .expect("fixture corner indices must be valid");
        cloth
    }

    fn shear_fixture(shear_compliance: f64) -> Cloth {
        Cloth::rectangular(RectangularClothConfig {
            columns: 3,
            rows: 3,
            spacing: 0.2,
            particle_mass: 1.0,
            stretch_compliance: 1.0e3,
            shear_compliance,
            bending_compliance: 1.0e3,
        })
        .expect("shear fixture must be valid")
    }

    fn bending_fixture(bending_compliance: f64) -> Cloth {
        Cloth::rectangular(RectangularClothConfig {
            columns: 2,
            rows: 2,
            spacing: 0.2,
            particle_mass: 1.0,
            stretch_compliance: 1.0e3,
            shear_compliance: 1.0e3,
            bending_compliance,
        })
        .expect("bending fixture must be valid")
    }

    fn sphere_fixture() -> ClothCollider {
        ClothCollider::Sphere(SphereCollider {
            center: Vec3::new(0.5, -0.5, 0.5),
            radius: 0.35,
            thickness: 0.02,
        })
    }

    fn capsule_fixture() -> ClothCollider {
        ClothCollider::Capsule(CapsuleCollider {
            start: Vec3::new(0.2, -0.5, 0.5),
            end: Vec3::new(0.8, -0.5, 0.5),
            radius: 0.3,
            thickness: 0.02,
        })
    }

    #[test]
    fn rectangular_topology_has_stable_ordering() {
        let cloth = cloth_fixture();

        assert_eq!(cloth.particles().len(), 36);
        assert_eq!(cloth.triangles().len(), 50);
        assert_eq!(cloth.stretch_constraints().len(), 60);
        assert_eq!(cloth.shear_constraints().len(), 50);
        assert_eq!(cloth.bending_constraints().len(), 65);
        assert_eq!(cloth.triangles()[0], [0, 6, 1]);
        assert_eq!(cloth.triangles()[49], [29, 34, 35]);
        assert_eq!(cloth.shear_constraints()[0].particle_a, 0);
        assert_eq!(cloth.shear_constraints()[0].particle_b, 7);
        assert_eq!(cloth.shear_constraints()[1].particle_a, 1);
        assert_eq!(cloth.shear_constraints()[1].particle_b, 6);
        assert_eq!(cloth.shear_constraints()[49].particle_a, 29);
        assert_eq!(cloth.shear_constraints()[49].particle_b, 34);
        let first_bend = cloth.bending_constraints()[0];
        assert_eq!(
            (first_bend.opposite_a, first_bend.opposite_b),
            (0, 7)
        );
        assert_eq!((first_bend.edge_a, first_bend.edge_b), (6, 1));
        let last_bend = cloth.bending_constraints()[64];
        assert_eq!((last_bend.opposite_a, last_bend.opposite_b), (28, 35));
        assert_eq!((last_bend.edge_a, last_bend.edge_b), (34, 29));
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
            let first_report = first
                .step(FixedStepConfig::default())
                .expect("first deterministic step must succeed");
            let second_report = second
                .step(FixedStepConfig::default())
                .expect("second deterministic step must succeed");
            assert_eq!(first_report, second_report);
        }

        assert_eq!(first.particles(), second.particles());
        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
    }

    #[test]
    fn hanging_sheet_falls_and_keeps_constraint_errors_bounded() {
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
        assert!(cloth.max_shear_error() < 0.03);
        assert!(cloth.max_bending_error().is_finite());
    }

    #[test]
    fn shear_constraints_reduce_diagonal_distortion() {
        let mut resistant = shear_fixture(1.0e-8);
        let mut compliant = shear_fixture(1.0e3);
        let center = 4;

        for cloth in [&mut resistant, &mut compliant] {
            cloth.particles[center].position += Vec3::new(0.12, 0.0, 0.05);
            cloth.particles[center].previous_position = cloth.particles[center].position;
        }

        let step = FixedStepConfig {
            gravity: Vec3::ZERO,
            solver_iterations: 20,
            velocity_damping: 0.0,
            ..FixedStepConfig::default()
        };
        resistant
            .step(step)
            .expect("resistant shear fixture must solve");
        compliant
            .step(step)
            .expect("compliant shear fixture must solve");

        let resistant_error = resistant.max_shear_error();
        let compliant_error = compliant.max_shear_error();
        assert!(compliant_error > 0.01);
        assert!(resistant_error < compliant_error * 0.25);
    }

    #[test]
    fn bending_constraints_reduce_fold_energy() {
        let mut resistant = bending_fixture(1.0e-8);
        let mut compliant = bending_fixture(1.0e3);
        let lifted = 3;

        for cloth in [&mut resistant, &mut compliant] {
            cloth.particles[lifted].position += Vec3::new(0.0, 0.15, 0.0);
            cloth.particles[lifted].previous_position = cloth.particles[lifted].position;
        }

        let step = FixedStepConfig {
            gravity: Vec3::ZERO,
            solver_iterations: 20,
            velocity_damping: 0.0,
            ..FixedStepConfig::default()
        };
        resistant
            .step(step)
            .expect("resistant bending fixture must solve");
        compliant
            .step(step)
            .expect("compliant bending fixture must solve");

        let resistant_error = resistant.max_bending_error();
        let compliant_error = compliant.max_bending_error();
        assert!(compliant_error > 0.5);
        assert!(resistant_error < compliant_error * 0.01);
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

        let ClothCollider::Sphere(sphere) = collider else {
            unreachable!("sphere fixture must contain a sphere")
        };
        let minimum_distance = sphere.effective_radius() - 1.0e-9;
        for particle in first
            .particles()
            .iter()
            .filter(|particle| !particle.is_pinned())
        {
            assert!((particle.position() - sphere.center).length() >= minimum_distance);
        }
    }

    #[test]
    fn capsule_collision_preserves_shell_and_replay() {
        let collider = capsule_fixture();
        let mut first = cloth_fixture();
        let mut second = cloth_fixture();
        let mut observed_collision = false;

        for _ in 0..240 {
            let first_report = first
                .step_with_colliders(FixedStepConfig::default(), &[collider])
                .expect("capsule collision step must remain valid");
            let second_report = second
                .step_with_colliders(FixedStepConfig::default(), &[collider])
                .expect("replayed capsule collision step must remain valid");
            assert_eq!(first_report, second_report);
            observed_collision |= first_report.collision_projections > 0;
        }

        assert!(observed_collision);
        assert_eq!(first.particles(), second.particles());

        let ClothCollider::Capsule(capsule) = collider else {
            unreachable!("capsule fixture must contain a capsule")
        };
        let minimum_distance = capsule.effective_radius() - 1.0e-9;
        for particle in first
            .particles()
            .iter()
            .filter(|particle| !particle.is_pinned())
        {
            let axis_point =
                closest_point_on_segment(particle.position(), capsule.start, capsule.end);
            assert!((particle.position() - axis_point).length() >= minimum_distance);
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
    fn swept_capsule_collision_blocks_tunneling() {
        let collider = CapsuleCollider {
            start: Vec3::new(0.0, -0.5, 0.0),
            end: Vec3::new(0.0, 0.5, 0.0),
            radius: 0.25,
            thickness: 0.05,
        };
        let mut particle = Particle {
            position: Vec3::new(1.0, 0.0, 0.0),
            previous_position: Vec3::new(-1.0, 0.0, 0.0),
            inverse_mass: 1.0,
        };

        assert!(solve_capsule_collision(&mut particle, collider));
        assert!((particle.position.x + collider.effective_radius()).abs() < 1.0e-10);
        assert!(particle.position.y.abs() < 1.0e-10);
        assert!(particle.position.z.abs() < 1.0e-10);
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
    fn invalid_capsule_axis_fails_closed_before_advancing() {
        let mut cloth = cloth_fixture();
        let fingerprint = cloth.state_fingerprint();
        let invalid = ClothCollider::Capsule(CapsuleCollider {
            start: Vec3::ZERO,
            end: Vec3::ZERO,
            radius: 0.5,
            thickness: 0.0,
        });

        let error = cloth
            .step_with_colliders(FixedStepConfig::default(), &[invalid])
            .expect_err("degenerate capsule axis must be rejected");

        assert_eq!(error, ClothError::InvalidColliderAxis);
        assert_eq!(cloth.state_fingerprint(), fingerprint);
    }

    #[test]
    fn invalid_shear_compliance_is_rejected() {
        let error = Cloth::rectangular(RectangularClothConfig {
            columns: 2,
            rows: 2,
            spacing: 0.2,
            particle_mass: 1.0,
            stretch_compliance: 0.0,
            shear_compliance: -1.0,
            bending_compliance: 0.0,
        })
        .expect_err("negative shear compliance must be rejected");

        assert_eq!(error, ClothError::InvalidCompliance);
    }

    #[test]
    fn invalid_bending_compliance_is_rejected() {
        let error = Cloth::rectangular(RectangularClothConfig {
            columns: 2,
            rows: 2,
            spacing: 0.2,
            particle_mass: 1.0,
            stretch_compliance: 0.0,
            shear_compliance: 0.0,
            bending_compliance: -1.0,
        })
        .expect_err("negative bending compliance must be rejected");

        assert_eq!(error, ClothError::InvalidCompliance);
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
