//! Jump physics.
//!
//! Implements jump impulse application matching
//! `LivingEntity.jumpFromGround()` from vanilla.

use crate::entity::Entity;

use super::constants::*;

/// Applies a jump impulse to the entity.
///
/// Sets vertical velocity to [`JUMP_POWER`] (0.42 blocks/tick), modified
/// by `jump_boost_level` (0 = no effect, 1 = Jump Boost I, etc.) and
/// `jump_factor` (from block below, e.g. 0.5 for honey blocks).
///
/// If the entity is sprinting, also applies a horizontal boost of 0.2
/// in the entity's facing direction.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::Entity;
/// use oxidized_game::physics::jump::apply_jump;
/// use oxidized_protocol::types::resource_location::ResourceLocation;
///
/// let mut entity = Entity::new(ResourceLocation::minecraft("player"), 0.6, 1.8);
/// entity.on_ground = true;
/// apply_jump(&mut entity, 0, 1.0);
/// assert!((entity.vy - 0.42).abs() < 0.001);
/// ```
pub fn apply_jump(entity: &mut Entity, jump_boost_level: u8, jump_factor: f64) {
    let base = JUMP_POWER * jump_factor + f64::from(jump_boost_level) * JUMP_BOOST_PER_LEVEL;
    entity.vy = base;

    if entity.is_sprinting() {
        // Sprint-jump: horizontal boost in facing direction.
        let yaw_rad = f64::from(entity.yaw).to_radians();
        entity.vx -= yaw_rad.sin() * SPRINT_JUMP_BOOST;
        entity.vz += yaw_rad.cos() * SPRINT_JUMP_BOOST;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::entity::data_slots::FLAG_SPRINTING;
    use oxidized_protocol::types::resource_location::ResourceLocation;

    fn make_entity_on_ground() -> Entity {
        let mut e = Entity::new(ResourceLocation::minecraft("pig"), 0.6, 1.8);
        e.set_pos(0.5, 1.0, 0.5);
        e.on_ground = true;
        e
    }

    #[test]
    fn test_jump_no_boost() {
        let mut entity = make_entity_on_ground();
        apply_jump(&mut entity, 0, 1.0);
        assert!(
            (entity.vy - JUMP_POWER).abs() < 0.0001,
            "Jump vy {} ≠ {}",
            entity.vy,
            JUMP_POWER
        );
    }

    #[test]
    fn test_jump_with_boost_ii() {
        let mut entity = make_entity_on_ground();
        apply_jump(&mut entity, 2, 1.0);
        let expected = JUMP_POWER + 2.0 * JUMP_BOOST_PER_LEVEL;
        assert!(
            (entity.vy - expected).abs() < 0.0001,
            "Jump Boost II vy {} ≠ {}",
            entity.vy,
            expected
        );
    }

    #[test]
    fn test_jump_honey_block_factor() {
        let mut entity = make_entity_on_ground();
        apply_jump(&mut entity, 0, HONEY_BLOCK_JUMP_FACTOR);
        let expected = JUMP_POWER * HONEY_BLOCK_JUMP_FACTOR;
        assert!(
            (entity.vy - expected).abs() < 0.0001,
            "Honey jump vy {} ≠ {}",
            entity.vy,
            expected
        );
    }

    #[test]
    fn test_sprint_jump_adds_horizontal_boost() {
        let mut entity = make_entity_on_ground();
        entity.set_flag(FLAG_SPRINTING, true);
        entity.yaw = 0.0; // facing +Z
        entity.vx = 0.0;
        entity.vz = 0.0;

        apply_jump(&mut entity, 0, 1.0);

        // Facing 0° (yaw=0): sin(0)=0, cos(0)=1
        // vx -= 0 * 0.2 = 0, vz += 1 * 0.2 = 0.2
        assert!(
            entity.vx.abs() < 0.001,
            "vx should be ~0 at yaw=0: {}",
            entity.vx
        );
        assert!(
            (entity.vz - SPRINT_JUMP_BOOST).abs() < 0.001,
            "vz should be ~0.2 at yaw=0: {}",
            entity.vz
        );
    }

    #[test]
    fn test_no_sprint_boost_when_not_sprinting() {
        let mut entity = make_entity_on_ground();
        entity.yaw = 90.0;
        entity.vx = 0.0;
        entity.vz = 0.0;

        apply_jump(&mut entity, 0, 1.0);

        assert!(
            entity.vx.abs() < 0.0001,
            "No horizontal boost without sprint"
        );
        assert!(
            entity.vz.abs() < 0.0001,
            "No horizontal boost without sprint"
        );
    }
}
