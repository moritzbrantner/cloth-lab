use std::fmt::Write as _;

use cloth_lab::{
    CapsuleCollider, Cloth, ClothCollider, ClothError, FixedStepConfig, TextileParameters,
    TextilePreset, Vec3,
};

const COLUMNS: usize = 14;
const ROWS: usize = 12;
const SPACING: f64 = 0.16;
const DEMO_PRESET: TextilePreset = TextilePreset::CottonLike;
const DEMO_PARAMETERS: TextileParameters = DEMO_PRESET.parameters();
const DISPLAY_FRAMES: usize = 181;
const STEPS_PER_FRAME: usize = 2;
const CAPSULE: CapsuleCollider = CapsuleCollider {
    start: Vec3::new(0.52, -0.58, 0.88),
    end: Vec3::new(1.56, -0.58, 0.88),
    radius: 0.28,
    thickness: 0.025,
};
const FIXTURE_CAPSULE: CapsuleCollider = CapsuleCollider {
    start: Vec3::new(0.2, -0.5, 0.5),
    end: Vec3::new(0.8, -0.5, 0.5),
    radius: 0.3,
    thickness: 0.02,
};

#[derive(Clone, Copy)]
struct FrameEvidence {
    max_stretch_error: f64,
    max_shear_error: f64,
    max_bending_error: f64,
    collision_projections: usize,
    friction_corrections: usize,
}

#[derive(Clone, Copy)]
struct PresetEvidence {
    fingerprint: u64,
    bottom_middle_y: f64,
    max_stretch_error: f64,
    max_shear_error: f64,
    max_bending_error: f64,
    collision_projections: usize,
    friction_corrections: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cloth = Cloth::rectangular(DEMO_PARAMETERS.rectangular_config(
        COLUMNS, COLUMNS.min(ROWS), SPACING, 1.0,
    ))?;
    cloth.pin_top_corners()?;

    let step = FixedStepConfig::default();
    let contact = DEMO_PARAMETERS.contact_config();
    let colliders = [ClothCollider::Capsule(CAPSULE)];
    let mut output = String::with_capacity(1_100_000);
    write!(
        &mut output,
        "{{\"columns\":{COLUMNS},\"rows\":{ROWS},\"spacing\":{SPACING:.8},\"materialPreset\":\"{}\",\"materialParameters\":{{\"stretchCompliance\":{:.12},\"shearCompliance\":{:.12},\"bendingCompliance\":{:.12},\"frictionCoefficient\":{:.8}}},\"stepsPerFrame\":{STEPS_PER_FRAME},\"deltaSeconds\":{:.17},\"presetSummaries\":[",
        DEMO_PRESET.name(),
        DEMO_PARAMETERS.stretch_compliance,
        DEMO_PARAMETERS.shear_compliance,
        DEMO_PARAMETERS.bending_compliance,
        DEMO_PARAMETERS.friction_coefficient,
        step.delta_seconds,
    )?;

    for (index, preset) in TextilePreset::ALL.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_preset_summary(&mut output, preset, run_preset_fixture(preset)?)?;
    }

    write!(
        &mut output,
        "],\"capsule\":{{\"start\":[{:.8},{:.8},{:.8}],\"end\":[{:.8},{:.8},{:.8}],\"radius\":{:.8},\"thickness\":{:.8}}},\"triangles\":[",
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

fn run_preset_fixture(preset: TextilePreset) -> Result<PresetEvidence, ClothError> {
    let parameters = preset.parameters();
    let mut cloth = Cloth::rectangular(parameters.rectangular_config(6, 6, 0.2, 1.0))?;
    cloth.pin_top_corners()?;

    let step = FixedStepConfig::default();
    let contact = parameters.contact_config();
    let colliders = [ClothCollider::Capsule(FIXTURE_CAPSULE)];
    let mut report = cloth.step_with_contacts(step, &colliders, contact)?;
    let mut collision_projections = report.collision_projections;
    let mut friction_corrections = report.friction_corrections;
    for _ in 1..240 {
        report = cloth.step_with_contacts(step, &colliders, contact)?;
        collision_projections += report.collision_projections;
        friction_corrections += report.friction_corrections;
    }

    let bottom_middle = (cloth.rows() - 1) * cloth.columns() + cloth.columns() / 2;
    Ok(PresetEvidence {
        fingerprint: cloth.state_fingerprint(),
        bottom_middle_y: cloth.particles()[bottom_middle].position().y,
        max_stretch_error: report.max_stretch_error,
        max_shear_error: report.max_shear_error,
        max_bending_error: report.max_bending_error,
        collision_projections,
        friction_corrections,
    })
}

fn write_preset_summary(
    output: &mut String,
    preset: TextilePreset,
    evidence: PresetEvidence,
) -> std::fmt::Result {
    let parameters = preset.parameters();
    write!(
        output,
        "{{\"name\":\"{}\",\"stretchCompliance\":{:.12},\"shearCompliance\":{:.12},\"bendingCompliance\":{:.12},\"frictionCoefficient\":{:.8},\"fingerprint\":\"{:016x}\",\"bottomMiddleY\":{:.12},\"maxStretchError\":{:.12},\"maxShearError\":{:.12},\"maxBendingError\":{:.12},\"collisionProjections\":{},\"frictionCorrections\":{}}}",
        preset.name(),
        parameters.stretch_compliance,
        parameters.shear_compliance,
        parameters.bending_compliance,
        parameters.friction_coefficient,
        evidence.fingerprint,
        evidence.bottom_middle_y,
        evidence.max_stretch_error,
        evidence.max_shear_error,
        evidence.max_bending_error,
        evidence.collision_projections,
        evidence.friction_corrections,
    )
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
