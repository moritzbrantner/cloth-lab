use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleDrag {
    particle_index: usize,
    restored_inverse_mass: f64,
}

impl ParticleDrag {
    #[must_use]
    pub const fn particle_index(self) -> usize {
        self.particle_index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClothInteractionError {
    InvalidParticleIndex,
    InvalidTarget,
    AlreadyKinematic,
    DragStateMismatch,
}

impl fmt::Display for ClothInteractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidParticleIndex => "particle index is outside the cloth",
            Self::InvalidTarget => "interactive particle target must contain only finite components",
            Self::AlreadyKinematic => "particle is already pinned or controlled kinematically",
            Self::DragStateMismatch => "particle is no longer controlled by this drag operation",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ClothInteractionError {}

impl Cloth {
    pub fn begin_particle_drag(
        &mut self,
        particle_index: usize,
    ) -> Result<ParticleDrag, ClothInteractionError> {
        begin_particle_drag(&mut self.particles, particle_index)
    }

    pub fn update_particle_drag(
        &mut self,
        drag: ParticleDrag,
        target: Vec3,
    ) -> Result<(), ClothInteractionError> {
        update_particle_drag(&mut self.particles, drag, target)
    }

    pub fn end_particle_drag(
        &mut self,
        drag: ParticleDrag,
    ) -> Result<(), ClothInteractionError> {
        end_particle_drag(&mut self.particles, drag)
    }
}

impl TriangleMeshCloth {
    pub fn begin_particle_drag(
        &mut self,
        particle_index: usize,
    ) -> Result<ParticleDrag, ClothInteractionError> {
        begin_particle_drag(&mut self.particles, particle_index)
    }

    pub fn update_particle_drag(
        &mut self,
        drag: ParticleDrag,
        target: Vec3,
    ) -> Result<(), ClothInteractionError> {
        update_particle_drag(&mut self.particles, drag, target)
    }

    pub fn end_particle_drag(
        &mut self,
        drag: ParticleDrag,
    ) -> Result<(), ClothInteractionError> {
        end_particle_drag(&mut self.particles, drag)
    }
}

fn begin_particle_drag(
    particles: &mut [Particle],
    particle_index: usize,
) -> Result<ParticleDrag, ClothInteractionError> {
    let particle = particles
        .get_mut(particle_index)
        .ok_or(ClothInteractionError::InvalidParticleIndex)?;
    if particle.is_pinned() {
        return Err(ClothInteractionError::AlreadyKinematic);
    }

    let drag = ParticleDrag {
        particle_index,
        restored_inverse_mass: particle.inverse_mass,
    };
    particle.inverse_mass = 0.0;
    particle.previous_position = particle.position;
    Ok(drag)
}

fn update_particle_drag(
    particles: &mut [Particle],
    drag: ParticleDrag,
    target: Vec3,
) -> Result<(), ClothInteractionError> {
    if !target.is_finite() {
        return Err(ClothInteractionError::InvalidTarget);
    }
    let particle = particles
        .get_mut(drag.particle_index)
        .ok_or(ClothInteractionError::InvalidParticleIndex)?;
    if !particle.is_pinned() {
        return Err(ClothInteractionError::DragStateMismatch);
    }

    particle.position = target;
    particle.previous_position = target;
    Ok(())
}

fn end_particle_drag(
    particles: &mut [Particle],
    drag: ParticleDrag,
) -> Result<(), ClothInteractionError> {
    let particle = particles
        .get_mut(drag.particle_index)
        .ok_or(ClothInteractionError::InvalidParticleIndex)?;
    if !particle.is_pinned() {
        return Err(ClothInteractionError::DragStateMismatch);
    }

    particle.inverse_mass = drag.restored_inverse_mass;
    particle.previous_position = particle.position;
    Ok(())
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    fn rectangular_fixture() -> Cloth {
        Cloth::rectangular(RectangularClothConfig {
            columns: 2,
            rows: 2,
            spacing: 0.2,
            particle_mass: 2.0,
            stretch_compliance: 1.0e-7,
            shear_compliance: 2.5e-7,
            bending_compliance: 1.0e-3,
        })
        .expect("rectangular interaction fixture must build")
    }

    fn mesh_fixture() -> TriangleMeshCloth {
        TriangleMeshCloth::new(
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.2, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.2),
            ],
            &[[0, 2, 1]],
            TriangleMeshClothConfig {
                particle_mass: 2.0,
                stretch_compliance: 1.0e-7,
                bending_compliance: 1.0e-3,
            },
        )
        .expect("triangle-mesh interaction fixture must build")
    }

    #[test]
    fn rectangular_drag_target_is_kinematic_until_release() {
        let mut cloth = rectangular_fixture();
        let original_inverse_mass = cloth.particles()[3].inverse_mass();
        let drag = cloth.begin_particle_drag(3).expect("drag must start");
        let target = Vec3::new(0.4, 0.3, 0.5);

        cloth
            .update_particle_drag(drag, target)
            .expect("drag target must update");
        cloth.step(FixedStepConfig::default()).unwrap();

        assert_eq!(cloth.particles()[3].position(), target);
        assert_eq!(cloth.particles()[3].inverse_mass(), 0.0);

        cloth.end_particle_drag(drag).expect("drag must end");
        assert_eq!(cloth.particles()[3].inverse_mass(), original_inverse_mass);
        assert_eq!(cloth.particles()[3].previous_position(), target);
    }

    #[test]
    fn triangle_mesh_drag_uses_identical_input_semantics() {
        let mut cloth = mesh_fixture();
        let original_inverse_mass = cloth.particles()[2].inverse_mass();
        let drag = cloth.begin_particle_drag(2).expect("drag must start");
        let target = Vec3::new(-0.2, 0.4, 0.3);

        cloth
            .update_particle_drag(drag, target)
            .expect("drag target must update");
        cloth.step(FixedStepConfig::default()).unwrap();

        assert_eq!(cloth.particles()[2].position(), target);
        cloth.end_particle_drag(drag).expect("drag must end");
        assert_eq!(cloth.particles()[2].inverse_mass(), original_inverse_mass);
        assert_eq!(cloth.particles()[2].previous_position(), target);
    }

    #[test]
    fn drag_replay_is_deterministic() {
        let mut first = mesh_fixture();
        let mut second = mesh_fixture();
        let first_drag = first.begin_particle_drag(1).unwrap();
        let second_drag = second.begin_particle_drag(1).unwrap();
        let targets = [
            Vec3::new(0.2, 0.1, 0.0),
            Vec3::new(0.25, 0.15, -0.05),
            Vec3::new(0.3, 0.2, -0.1),
        ];

        for target in targets {
            first.update_particle_drag(first_drag, target).unwrap();
            second.update_particle_drag(second_drag, target).unwrap();
            assert_eq!(
                first.step(FixedStepConfig::default()).unwrap(),
                second.step(FixedStepConfig::default()).unwrap()
            );
        }
        first.end_particle_drag(first_drag).unwrap();
        second.end_particle_drag(second_drag).unwrap();

        assert_eq!(first.particles(), second.particles());
        assert_eq!(first.state_fingerprint(), second.state_fingerprint());
    }

    #[test]
    fn invalid_target_fails_closed() {
        let mut cloth = mesh_fixture();
        let drag = cloth.begin_particle_drag(1).unwrap();
        let before = cloth.particles()[1];

        let error = cloth
            .update_particle_drag(drag, Vec3::new(f64::NAN, 0.0, 0.0))
            .expect_err("non-finite target must fail");

        assert_eq!(error, ClothInteractionError::InvalidTarget);
        assert_eq!(cloth.particles()[1], before);
    }

    #[test]
    fn pinned_particle_cannot_start_nested_drag() {
        let mut cloth = rectangular_fixture();
        cloth.pin(0).unwrap();

        let error = cloth
            .begin_particle_drag(0)
            .expect_err("pinned particle must not start a drag session");

        assert_eq!(error, ClothInteractionError::AlreadyKinematic);
    }
}
