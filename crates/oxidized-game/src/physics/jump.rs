//! Jump physics.
//!
//! Implements jump impulse application matching
//! `LivingEntity.jumpFromGround()` from vanilla.

use crate::entity::components::Velocity;

use super::constants::*;

/// Applies a jump impulse to the entity.
///
/// Sets vertical velocity to [`JUMP_POWER`] (0.42 blocks/tick), modified
/// by `jump_boost_level` (0 = no effect, 1 = Jump Boost I, etc.) and
/// `jump_factor` (from block below, e.g. 0.5 for honey blocks).
///
/// If `is_sprinting` is true, also applies a horizontal boost of 0.2
/// in the entity's facing direction (based on `yaw`).
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::components::Velocity;
/// use oxidized_game::physics::jump::apply_jump;
/// use glam::DVec3;
///
/// let mut vel = Velocity(DVec3::ZERO);
/// apply_jump(&mut vel, 0.0, false, 0, 1.0);
/// assert!((vel.0.y - 0.42).abs() < 0.001);
/// ```
pub fn apply_jump(
    vel: &mut Velocity,
    yaw: f32,
    is_sprinting: bool,
    jump_boost_level: u8,
    jump_factor: f64,
) {
    let base = JUMP_POWER * jump_factor + f64::from(jump_boost_level) * JUMP_BOOST_PER_LEVEL;
    vel.0.y = base;

    if is_sprinting {
        // Sprint-jumping: horizontal boost in facing direction.
        let yaw_rad = f64::from(yaw).to_radians();
        vel.0.x -= yaw_rad.sin() * SPRINT_JUMP_BOOST;
        vel.0.z += yaw_rad.cos() * SPRINT_JUMP_BOOST;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use glam::DVec3;

    #[test]
    fn test_jump_no_boost() {
        let mut vel = Velocity(DVec3::ZERO);
        apply_jump(&mut vel, 0.0, false, 0, 1.0);
        assert!(
            (vel.0.y - JUMP_POWER).abs() < 0.0001,
            "Jump vy {} ≠ {}",
            vel.0.y,
            JUMP_POWER
        );
    }

    #[test]
    fn test_jump_with_boost_ii() {
        let mut vel = Velocity(DVec3::ZERO);
        apply_jump(&mut vel, 0.0, false, 2, 1.0);
        let expected = JUMP_POWER + 2.0 * JUMP_BOOST_PER_LEVEL;
        assert!(
            (vel.0.y - expected).abs() < 0.0001,
            "Jump Boost II vy {} ≠ {}",
            vel.0.y,
            expected
        );
    }

    #[test]
    fn test_jump_honey_block_factor() {
        let mut vel = Velocity(DVec3::ZERO);
        // Honey block jump factor is 0.5 (from block registry).
        apply_jump(&mut vel, 0.0, false, 0, 0.5);
        let expected = JUMP_POWER * 0.5;
        assert!(
            (vel.0.y - expected).abs() < 0.0001,
            "Honey jump vy {} ≠ {}",
            vel.0.y,
            expected
        );
    }

    #[test]
    fn test_sprint_jump_adds_horizontal_boost() {
        let mut vel = Velocity(DVec3::ZERO);
        apply_jump(&mut vel, 0.0, true, 0, 1.0);

        // Facing 0° (yaw=0): sin(0)=0, cos(0)=1
        // vx -= 0 * 0.2 = 0, vz += 1 * 0.2 = 0.2
        assert!(
            vel.0.x.abs() < 0.001,
            "vx should be ~0 at yaw=0: {}",
            vel.0.x
        );
        assert!(
            (vel.0.z - SPRINT_JUMP_BOOST).abs() < 0.001,
            "vz should be ~0.2 at yaw=0: {}",
            vel.0.z
        );
    }

    #[test]
    fn test_no_sprint_boost_when_not_sprinting() {
        let mut vel = Velocity(DVec3::ZERO);
        apply_jump(&mut vel, 90.0, false, 0, 1.0);

        assert!(
            vel.0.x.abs() < 0.0001,
            "No horizontal boost without sprint"
        );
        assert!(
            vel.0.z.abs() < 0.0001,
            "No horizontal boost without sprint"
        );
    }
}
