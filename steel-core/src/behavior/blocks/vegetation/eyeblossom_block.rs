use glam::DVec3;
use rand::RngExt;
use std::sync::Arc;

use steel_macros::block_behavior;
use steel_registry::blocks::block_state_ext::BlockStateExt;
use steel_registry::mob_effect::MobEffectRef;
use steel_registry::particle_type::{ParticleData, TrailParticleOption};
use steel_registry::sound_event::SoundEventRef;
use steel_registry::vanilla_block_tags::BlockTag;
use steel_registry::vanilla_particle_types;
use steel_registry::{
    sound_events, vanilla_blocks, vanilla_entities, vanilla_game_events, vanilla_mob_effects,
};
use steel_utils::types::{Difficulty, UpdateFlags};
use steel_utils::{BlockPos, BlockStateId, RgbColor};

use crate::behavior::block::BlockBehavior;
use crate::behavior::context::BlockPlaceContext;
use crate::entity::{Entity, InsideBlockEffectCollector, MobEffectInstance};
use crate::world::game_event::GameEventContext;
use crate::world::{LevelReader, World};

use super::{BlockRef, default_surviving_state, survives_on_tag};

pub(crate) const EYEBLOSSOM_OPEN_ATTRIBUTE: &str = "minecraft:gameplay/eyeblossom_open";

#[derive(Clone, Copy, PartialEq, Eq)]
/// Vanilla open/closed eyeblossom type from `classes.json`.
pub enum EyeblossomType {
    /// Emits open-eyeblossom effects and transforms closed at daytime.
    Open,
    /// Emits closed-eyeblossom effects and transforms open at nighttime.
    Closed,
}

impl EyeblossomType {
    pub(crate) const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    pub(crate) const fn transform(self) -> Self {
        match self {
            Self::Open => Self::Closed,
            Self::Closed => Self::Open,
        }
    }

    pub(crate) const fn block(self) -> BlockRef {
        match self {
            Self::Open => &vanilla_blocks::OPEN_EYEBLOSSOM,
            Self::Closed => &vanilla_blocks::CLOSED_EYEBLOSSOM,
        }
    }

    pub(crate) const fn long_switch_sound(self) -> SoundEventRef {
        match self {
            Self::Open => &sound_events::BLOCK_EYEBLOSSOM_OPEN_LONG,
            Self::Closed => &sound_events::BLOCK_EYEBLOSSOM_CLOSE_LONG,
        }
    }

    const fn short_switch_sound(self) -> SoundEventRef {
        match self {
            Self::Open => &sound_events::BLOCK_EYEBLOSSOM_OPEN,
            Self::Closed => &sound_events::BLOCK_EYEBLOSSOM_CLOSE,
        }
    }

    pub(crate) const fn particle_color(self) -> RgbColor {
        match self {
            Self::Open => RgbColor::new(16_545_810),
            Self::Closed => RgbColor::new(6_250_335),
        }
    }

    pub(crate) const fn bee_effect(self) -> MobEffectRef {
        match self {
            Self::Open => &vanilla_mob_effects::BLINDNESS,
            Self::Closed => &vanilla_mob_effects::NAUSEA,
        }
    }

    pub(crate) const fn bee_effect_duration(self) -> i32 {
        match self {
            Self::Open => 220,
            Self::Closed => 140,
        }
    }

    pub(crate) fn spawn_transform_particle(self, world: &Arc<World>, pos: BlockPos) {
        let mut rng = rand::rng();
        let start = DVec3::new(
            f64::from(pos.x()) + 0.5,
            f64::from(pos.y()) + 0.5,
            f64::from(pos.z()) + 0.5,
        );
        let lifetime = 0.5 + rng.random::<f64>();
        let velocity = DVec3::new(
            rng.random::<f64>() - 0.5,
            rng.random::<f64>() + 1.0,
            rng.random::<f64>() - 0.5,
        );
        let target = start + velocity * lifetime;
        let duration = (20.0 * lifetime) as i32;
        let particle = ParticleData::new(
            &vanilla_particle_types::TRAIL,
            TrailParticleOption::new(target, self.particle_color(), duration),
        );
        world.send_particles(particle, start, 1, DVec3::ZERO, 0.0);
    }
}

#[block_behavior]
/// Behavior for open and closed Eyeblossom blocks.
pub struct EyeblossomBlock {
    block: BlockRef,
    #[json_arg(r#enum = "EyeblossomType", json = "type")]
    eyeblossom_type: EyeblossomType,
}

impl EyeblossomBlock {
    /// Creates a new eyeblossom behavior.
    #[must_use]
    pub const fn new(block: BlockRef, eyeblossom_type: EyeblossomType) -> Self {
        Self {
            block,
            eyeblossom_type,
        }
    }

    fn try_changing_state(
        &self,
        state: BlockStateId,
        world: &Arc<World>,
        pos: BlockPos,
        long_sound: bool,
    ) {
        let should_be_open = world
            .environment_bool_attribute(EYEBLOSSOM_OPEN_ATTRIBUTE, self.eyeblossom_type.is_open());
        if should_be_open == self.eyeblossom_type.is_open() {
            return;
        }

        let new_type = self.eyeblossom_type.transform();
        let new_state = new_type.block().default_state();
        if !world.set_block(pos, new_state, UpdateFlags::UPDATE_ALL) {
            return;
        }

        world.game_event(
            &vanilla_game_events::BLOCK_CHANGE,
            pos,
            &GameEventContext::new(None, Some(state)),
        );
        new_type.spawn_transform_particle(world, pos);
        let sound = if long_sound {
            new_type.long_switch_sound()
        } else {
            new_type.short_switch_sound()
        };
        world.play_block_sound(sound, pos, 1.0, 1.0, None);

        let mut rng = rand::rng();
        for nearby_pos in BlockPos::between_closed(pos.offset(-3, -2, -3), pos.offset(3, 2, 3)) {
            if world.get_block_state(nearby_pos) != state {
                continue;
            }

            let dx = f64::from(nearby_pos.x() - pos.x());
            let dy = f64::from(nearby_pos.y() - pos.y());
            let dz = f64::from(nearby_pos.z() - pos.z());
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            let min_delay = (distance * 5.0) as i32;
            let max_delay = (distance * 10.0) as i32;
            let delay = rng.random_range(min_delay..=max_delay);
            world.schedule_block_tick_default(nearby_pos, state.get_block(), delay);
        }
    }
}

impl BlockBehavior for EyeblossomBlock {
    fn can_survive(&self, _state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> bool {
        survives_on_tag(world, pos, &BlockTag::SUPPORTS_VEGETATION)
    }

    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        default_surviving_state(self.block, self, context)
    }

    // Vanilla's animateTick idle sound is client-local, so it is emitted by the client rather
    // than broadcast from the server block behavior.
    fn random_tick(&self, state: BlockStateId, world: &Arc<World>, pos: BlockPos) {
        self.try_changing_state(state, world, pos, true);
    }

    fn tick(&self, state: BlockStateId, world: &Arc<World>, pos: BlockPos) {
        self.try_changing_state(state, world, pos, false);
    }

    fn entity_inside(
        &self,
        state: BlockStateId,
        world: &Arc<World>,
        _pos: BlockPos,
        entity: &dyn Entity,
        _effect_collector: &mut InsideBlockEffectCollector,
        _is_precise: bool,
    ) {
        if world.difficulty() == Difficulty::Peaceful
            || !state.get_block().has_tag(&BlockTag::BEE_ATTRACTIVE)
            || entity.entity_type() != &vanilla_entities::BEE
        {
            return;
        }

        let Some(living_entity) = entity.as_living_entity() else {
            return;
        };
        if living_entity.has_mob_effect(vanilla_mob_effects::POISON) {
            return;
        }
        living_entity.add_mob_effect(MobEffectInstance::with_duration(
            self.eyeblossom_type.bee_effect(),
            self.eyeblossom_type.bee_effect_duration(),
            0,
        ));
    }
}

#[cfg(test)]
mod tests {
    use steel_registry::{init_vanilla_registry, vanilla_blocks};
    use steel_utils::BlockPos;

    use crate::test_support::TestLevel;

    use super::*;

    fn level_with_support(support: BlockRef) -> TestLevel {
        TestLevel::default().with_block(BlockPos::new(0, 63, 0), support.default_state())
    }

    #[test]
    fn eyeblossom_requires_vegetation_support() {
        init_vanilla_registry();
        let behavior =
            EyeblossomBlock::new(&vanilla_blocks::CLOSED_EYEBLOSSOM, EyeblossomType::Closed);
        let pos = BlockPos::new(0, 64, 0);
        let state = vanilla_blocks::CLOSED_EYEBLOSSOM.default_state();

        assert!(behavior.can_survive(state, &level_with_support(&vanilla_blocks::DIRT), pos));
        assert!(!behavior.can_survive(state, &level_with_support(&vanilla_blocks::AIR), pos));
    }
}
