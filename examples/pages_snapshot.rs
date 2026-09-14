use std::fmt::Write as _;

use cloth_lab::{
    CapsuleCollider, Cloth, ClothCollider, ContactConfig, FixedStepConfig, RectangularClothConfig,
    Vec3,
};

const COLUMNS: usize = 14;
const ROWS: usize = 12;
const SPACING: f64 = 0.16;
const STRETCH_COMPLIANCE: f64 = 1.0e-7;
const SHEAR_COMPLIANCE: f64 = 2.5e-7;
const BENDING_COMPLIANCE: f64 = 1.0e-3;
const FRICTION_COEFFICIENT: f64 = 0.55;
const DISPLAY_FRAMES: usize = 181;
const STEPS_PER_FRAME: usize = 2;
const CAPSULE: CapsuleCollider = CapsuleCollider {
    start: Vec3::new(0.52, -0.58, 0.88),
    end: Vec3::new(1.56, -0.58, 0.88),
    radius: 0.28,
    thickness: 0.025,
};

#[derive(Clone, Copy)]
struct FrameEvidence {
    max_stretch_error: f64,
    max_shear_error: f64,
    max_bending_error: f64,
    collision_projections: usize,
    friction_corrections: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cloth = Cloth::rectangular(RectangularClothConfig {
        columns: COLUMNS,
        rows: ROWS,
        spacing: SPACING,
        particle_mass: 1.0,
        stretch_compliance: STRETCH_COMPLIANCE,
        shear_compliance: SHEAR_COMPLIANCE,
        bending_compliance: BENDING_COMPLIANCE,
    })?;
    cloth.pin_top_corners()?;

    let step = FixedStepConfig::default();
    let contact = ContactConfig {
        friction_coefficient: FRICTION_COEFFICIENT,
    };
    let colliders = [ClothCollider::Capsule(CAPSULE)];
    let mut output = String::with_capacity(1_000_000);
    write!(
        &mut output,
        "{{\"columns\":{COLUMNS},\"rows\":{ROWS},\"spacing\":{SPACING:.8},\"stretchCompliance\":{STRETCH_COMPLIANCE:.12},\"shearCompliance\":{SHEAR_COMPLIANCE:.12},\"bendingCompliance\":{BENDING_COMPLIANCE:.12},\"frictionCoefficient\":{FRICTION_COEFFICIENT:.8},\"stepsPerFrame\":{STEPS_PER_FRAME},\"deltaSeconds\":{:.17},\"capsule\":{{\"start\":[{:.8},{:.8},{:.8}],\"end\":[{:.8},{:.8},{:.8}],\"radius\":{:.8},\"thickness\":{:.8}}},\"triangles\":[",
        step.delta_seconds,
        CAPSULE.start.x,
        CAPSULE.start.y,
        CAPSULE.start.z,
        CAPSULE.end.x,
        CAPSULE.end.y,
        CAPSULE.end.z,
        CAPSULE.radius,
        CAPSULE.thickness
    )?;

    for (index, triangle) in cloth.triangles().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(
            &mut output,
            "[{},{},{}]",
            triangle[0], triangle[1], triangle[2]
        )?;
    }

    output.push_str("],\"pinned\":[");
    let mut first_pinned = true;
    for (index, particle) in cloth.particles().iter().enumerate() {
        if particle.is_pinned() {
            if !first_pinned {
                output.push(',');
            }
            write!(&mut output, "{index}")?;
            first_pinned = false;
        }
    }

    output.push_str("],\"frames\":[");
    write_frame(
        &mut output,
        0,
        &cloth,
        FrameEvidence {
            max_stretch_error: cloth.max_stretch_error(),
            max_shear_error: cloth.max_shear_error(),
            max_bending_error: cloth.max_bending_error(),
            collision_projections: 0,
            friction_corrections: 0,
        },
    )?;

    for frame_index in 1..DISPLAY_FRAMES {
        let mut report = cloth.step_with_contacts(step, &colliders, contact)?;
        for _ in 1..STEPS_PER_FRAME {
            report = cloth.step_with_contacts(step, &colliders, contact)?;
        }

        output.push(',');
        write_frame(
            &mut output,
            frame_index * STEPS_PER_FRAME,
            &cloth,
            FrameEvidence {
                max_stretch_error: report.max_stretch_error,
                max_shear_error: report.max_shear_error,
                max_bending_error: report.max_bending_error,
                collision_projections: report.collision_projections,
                friction_corrections: report.friction_corrections,
            },
        )?;
    }

    output.push_str("]}");
    print!("{output}");
    Ok(())
}

fn write_frame(
    output: &mut String,
    step: usize,
    cloth: &Cloth,
    evidence: FrameEvidence,
) -> std::fmt::Result {
    write!(
        output,
        "{{\"step\":{step},\"fingerprint\":\"{:016x}\",\"maxStretchError\":{:.12},\"maxShearError\":{:.12},\"maxBendingError\":{:.12},\"collisionProjections\":{},\"frictionCorrections\":{},\"positions\":[",
        cloth.state_fingerprint(),
        evidence.max_stretch_error,
        evidence.max_shear_error,
        evidence.max_bending_error,
        evidence.collision_projections,
        evidence.friction_corrections,
    )?;

    for (index, particle) in cloth.particles().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let position = particle.position();
        write!(
            output,
            "[{:.8},{:.8},{:.8}]",
            position.x, position.y, position.z
        )?;
    }

    output.push_str("]}");
    Ok(())
}
