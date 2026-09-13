use std::fmt::Write as _;

use cloth_lab::{Cloth, FixedStepConfig, RectangularClothConfig};

const COLUMNS: usize = 14;
const ROWS: usize = 12;
const SPACING: f64 = 0.16;
const DISPLAY_FRAMES: usize = 181;
const STEPS_PER_FRAME: usize = 2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cloth = Cloth::rectangular(RectangularClothConfig {
        columns: COLUMNS,
        rows: ROWS,
        spacing: SPACING,
        particle_mass: 1.0,
        stretch_compliance: 1.0e-7,
    })?;
    cloth.pin_top_corners()?;

    let step = FixedStepConfig::default();
    let mut output = String::with_capacity(1_000_000);
    write!(
        &mut output,
        "{{\"columns\":{COLUMNS},\"rows\":{ROWS},\"spacing\":{SPACING:.8},\"stepsPerFrame\":{STEPS_PER_FRAME},\"deltaSeconds\":{:.17},\"triangles\":[",
        step.delta_seconds
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
    write_frame(&mut output, 0, &cloth, cloth.max_stretch_error())?;

    for frame_index in 1..DISPLAY_FRAMES {
        let mut report = cloth.step(step)?;
        for _ in 1..STEPS_PER_FRAME {
            report = cloth.step(step)?;
        }

        output.push(',');
        write_frame(
            &mut output,
            frame_index * STEPS_PER_FRAME,
            &cloth,
            report.max_stretch_error,
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
    max_stretch_error: f64,
) -> std::fmt::Result {
    write!(
        output,
        "{{\"step\":{step},\"fingerprint\":\"{:016x}\",\"maxStretchError\":{max_stretch_error:.12},\"positions\":[",
        cloth.state_fingerprint()
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
