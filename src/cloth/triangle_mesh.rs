use std::collections::BTreeSet;

const TRIANGLE_MESH_FINGERPRINT_MARKER: u64 = u64::MAX;

/// Solver parameters for an arbitrary indexed triangle surface.
///
/// A plain triangle mesh does not encode warp/weft or pattern-space shear directions, so this
/// configuration intentionally exposes structural stretch and bending only. Richer garment
/// importers can add explicit material directions later instead of guessing them from tessellation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriangleMeshClothConfig {
    pub particle_mass: f64,
    pub stretch_compliance: f64,
    pub bending_compliance: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriangleMeshClothError {
    EmptyMesh,
    InvalidParticleMass,
    InvalidCompliance,
    InvalidParticlePosition,
    InvalidTriangleIndex,
    DegenerateTriangle,
    NonManifoldTopology,
    InconsistentTriangleWinding,
    Solver(ClothError),
}

impl fmt::Display for TriangleMeshClothError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyMesh => "triangle-mesh cloth requires vertices and triangles",
            Self::InvalidParticleMass => "particle mass must be finite and positive",
            Self::InvalidCompliance => "constraint compliance must be finite and non-negative",
            Self::InvalidParticlePosition => {
                "triangle-mesh particle positions must contain only finite components"
            }
            Self::InvalidTriangleIndex => "triangle index is outside the cloth mesh",
            Self::DegenerateTriangle => {
                "cloth triangles must contain three distinct non-collinear vertices"
            }
            Self::NonManifoldTopology => {
                "cloth triangle mesh must have at most two triangles per edge"
            }
            Self::InconsistentTriangleWinding => {
                "adjacent cloth triangles must use opposite winding across their shared edge"
            }
            Self::Solver(error) => return fmt::Display::fmt(error, formatter),
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TriangleMeshClothError {}

#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMeshCloth {
    particles: Vec<Particle>,
    triangles: Vec<[usize; 3]>,
    stretch_constraints: Vec<DistanceConstraint>,
    bending_constraints: Vec<BendingConstraint>,
}

#[derive(Clone, Copy, Debug)]
struct TriangleMeshEdgeUse {
    start: usize,
    end: usize,
    count: u8,
}

impl TriangleMeshCloth {
    pub fn new(
        positions: &[Vec3],
        triangles: &[[usize; 3]],
        config: TriangleMeshClothConfig,
    ) -> Result<Self, TriangleMeshClothError> {
        validate_triangle_mesh(positions, triangles, config)?;

        let inverse_mass = 1.0 / config.particle_mass;
        let particles = positions
            .iter()
            .copied()
            .map(|position| Particle {
                position,
                previous_position: position,
                inverse_mass,
            })
            .collect::<Vec<_>>();
        let triangles = triangles.to_vec();
        let stretch_constraints = build_triangle_mesh_stretch_constraints(
            positions,
            &triangles,
            config.stretch_compliance,
        );
        let bending_constraints = build_bending_constraints(
            &particles,
            &triangles,
            config.bending_compliance,
        )
        .map_err(TriangleMeshClothError::Solver)?;

        Ok(Self {
            particles,
            triangles,
            stretch_constraints,
            bending_constraints,
        })
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
        &[]
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
            for constraint in &mut self.bending_constraints {
                solve_bending_constraint(particles, constraint, config.delta_seconds);
            }
            let collision_report = solve_collisions(particles, colliders, contact);
            collision_projections += collision_report.projections;
            friction_corrections += collision_report.friction_corrections;
        }

        Ok(StepReport {
            max_stretch_error: self.max_stretch_error(),
            max_shear_error: 0.0,
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
    pub const fn max_shear_error(&self) -> f64 {
        0.0
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
        hash_u64(&mut hash, TRIANGLE_MESH_FINGERPRINT_MARKER);
        hash_u64(&mut hash, self.triangles.len() as u64);
        for triangle in &self.triangles {
            for &index in triangle {
                hash_u64(&mut hash, index as u64);
            }
        }
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

fn validate_triangle_mesh(
    positions: &[Vec3],
    triangles: &[[usize; 3]],
    config: TriangleMeshClothConfig,
) -> Result<(), TriangleMeshClothError> {
    if positions.is_empty() || triangles.is_empty() {
        return Err(TriangleMeshClothError::EmptyMesh);
    }
    if !config.particle_mass.is_finite() || config.particle_mass <= 0.0 {
        return Err(TriangleMeshClothError::InvalidParticleMass);
    }
    if [config.stretch_compliance, config.bending_compliance]
        .into_iter()
        .any(|compliance| !compliance.is_finite() || compliance < 0.0)
    {
        return Err(TriangleMeshClothError::InvalidCompliance);
    }
    if positions.iter().any(|position| !position.is_finite()) {
        return Err(TriangleMeshClothError::InvalidParticlePosition);
    }

    let mut edge_uses = BTreeMap::<(usize, usize), TriangleMeshEdgeUse>::new();
    for &[a, b, c] in triangles {
        if [a, b, c].into_iter().any(|index| index >= positions.len()) {
            return Err(TriangleMeshClothError::InvalidTriangleIndex);
        }
        if a == b || b == c || a == c {
            return Err(TriangleMeshClothError::DegenerateTriangle);
        }

        let area_normal = cross(positions[b] - positions[a], positions[c] - positions[a]);
        if !area_normal.is_finite() || area_normal.length_squared() <= f64::EPSILON {
            return Err(TriangleMeshClothError::DegenerateTriangle);
        }

        for (start, end) in [(a, b), (b, c), (c, a)] {
            let key = triangle_mesh_ordered_edge(start, end);
            match edge_uses.get_mut(&key) {
                None => {
                    edge_uses.insert(
                        key,
                        TriangleMeshEdgeUse {
                            start,
                            end,
                            count: 1,
                        },
                    );
                }
                Some(edge_use) => {
                    if edge_use.count >= 2 {
                        return Err(TriangleMeshClothError::NonManifoldTopology);
                    }
                    if edge_use.start == start && edge_use.end == end {
                        return Err(TriangleMeshClothError::InconsistentTriangleWinding);
                    }
                    edge_use.count += 1;
                }
            }
        }
    }
    Ok(())
}

fn build_triangle_mesh_stretch_constraints(
    positions: &[Vec3],
    triangles: &[[usize; 3]],
    compliance: f64,
) -> Vec<DistanceConstraint> {
    let mut edges = BTreeSet::new();
    for &[a, b, c] in triangles {
        for (left, right) in [(a, b), (b, c), (c, a)] {
            edges.insert(triangle_mesh_ordered_edge(left, right));
        }
    }

    edges
        .into_iter()
        .map(|(particle_a, particle_b)| DistanceConstraint {
            particle_a,
            particle_b,
            rest_length: (positions[particle_a] - positions[particle_b]).length(),
            compliance,
            lambda: 0.0,
        })
        .collect()
}

fn triangle_mesh_ordered_edge(left: usize, right: usize) -> (usize, usize) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

#[cfg(test)]
mod triangle_mesh_tests {
    use super::*;
    use crate::{GarmentImporter, ObjGarmentImporter};

    fn triangle_mesh_fixture() -> TriangleMeshCloth {
        TriangleMeshCloth::new(
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
            ],
            &[[0, 2, 1], [1, 2, 3]],
            TriangleMeshClothConfig {
                particle_mass: 1.0,
                stretch_compliance: 1.0e-7,
                bending_compliance: 1.0e-3,
            },
        )
        .expect("triangle mesh fixture must be valid")
    }

    #[test]
    fn derives_only_real_mesh_edges() {
        let cloth = triangle_mesh_fixture();
        assert_eq!(cloth.particles().len(), 4);
        assert_eq!(cloth.triangles(), &[[0, 2, 1], [1, 2, 3]]);
        assert!(cloth.shear_constraints().is_empty());
        assert_eq!(cloth.bending_constraints().len(), 1);
        let edges = cloth
            .stretch_constraints()
            .iter()
            .map(|constraint| (constraint.particle_a, constraint.particle_b))
            .collect::<Vec<_>>();
        assert_eq!(edges, vec![(0, 1), (0, 2), (1, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn imported_obj_initializes_deterministic_triangle_mesh_cloth() {
        let source = b"v 0 0 0\nv 1 0 0\nv 0 0 1\nf 1 3 2\n";
        let asset = ObjGarmentImporter.import(source).expect("valid garment OBJ");
        let config = TriangleMeshClothConfig {
            particle_mass: 1.0,
            stretch_compliance: 1.0e-7,
            bending_compliance: 1.0e-3,
        };
        let first = TriangleMeshCloth::new(asset.positions(), asset.triangles(), config)
            .expect("imported garment must initialize cloth");
        let second = TriangleMeshCloth::new(asset.positions(), asset.triangles(), config)
            .expect("same imported garment must initialize cloth");

        assert_eq!(first.particles().len(), 3);
        assert_eq!(first.triangles(), asset.triangles());
        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
    }

    #[test]
    fn replay_is_deterministic() {
        let mut first = triangle_mesh_fixture();
        let mut second = triangle_mesh_fixture();
        first.pin(0).unwrap();
        second.pin(0).unwrap();
        for _ in 0..120 {
            assert_eq!(
                first.step(FixedStepConfig::default()).unwrap(),
                second.step(FixedStepConfig::default()).unwrap()
            );
        }
        assert_eq!(first.particles(), second.particles());
        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
    }

    #[test]
    fn malformed_meshes_fail_closed() {
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.5, 1.0, 0.5),
        ];
        let config = TriangleMeshClothConfig {
            particle_mass: 1.0,
            stretch_compliance: 0.0,
            bending_compliance: 0.0,
        };

        assert_eq!(
            TriangleMeshCloth::new(&positions[..3], &[[0, 1, 3]], config).unwrap_err(),
            TriangleMeshClothError::InvalidTriangleIndex
        );
        assert_eq!(
            TriangleMeshCloth::new(&positions[..3], &[[0, 1, 1]], config).unwrap_err(),
            TriangleMeshClothError::DegenerateTriangle
        );
        assert_eq!(
            TriangleMeshCloth::new(
                &[
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(2.0, 0.0, 0.0),
                ],
                &[[0, 1, 2]],
                config,
            )
            .unwrap_err(),
            TriangleMeshClothError::DegenerateTriangle
        );
        assert_eq!(
            TriangleMeshCloth::new(&positions[..4], &[[0, 1, 2], [0, 1, 3]], config)
                .unwrap_err(),
            TriangleMeshClothError::InconsistentTriangleWinding
        );
        assert_eq!(
            TriangleMeshCloth::new(
                &positions,
                &[[0, 1, 2], [1, 0, 3], [0, 1, 4]],
                config,
            )
            .unwrap_err(),
            TriangleMeshClothError::NonManifoldTopology
        );
    }
}
