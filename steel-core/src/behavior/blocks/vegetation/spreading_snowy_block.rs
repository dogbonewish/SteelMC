use std::sync::Arc;

use rand::RngExt;
use steel_registry::blocks::block_state_ext::BlockStateExt;
use steel_registry::blocks::properties::BlockStateProperties;
use steel_registry::fluid::FluidStateExt;
use steel_registry::vanilla_block_tags::BlockTag;
use steel_registry::{blocks::BlockRef, vanilla_blocks};
use steel_utils::{BlockPos, BlockStateId, Direction, types::UpdateFlags};

use crate::behavior::context::BlockPlaceContext;
use crate::chunk::light::get_light_block_into;
use crate::world::{LevelReader, World};

use super::snowy_block::{snowy_placement_state, update_snowy_shape};

/// Shared random-tick behavior for grass and mycelium.
pub(super) struct SpreadingSnowyBlock {
    block: BlockRef,
    base_block: BlockRef,
}

impl SpreadingSnowyBlock {
    /// Creates a spreading snowy block with its dirt-like fallback block.
    #[must_use]
    pub(super) const fn new(block: BlockRef, base_block: BlockRef) -> Self {
        Self { block, base_block }
    }

    pub(super) fn is_block(&self, block: BlockRef) -> bool {
        self.block == block
    }

    pub(super) fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> BlockStateId {
        snowy_placement_state(self.block, context)
    }

    pub(super) fn update_shape(
        state: BlockStateId,
        direction: Direction,
        neighbor_state: BlockStateId,
    ) -> BlockStateId {
        update_snowy_shape(state, direction, neighbor_state)
    }

    pub(super) fn random_tick(&self, state: BlockStateId, world: &Arc<World>, pos: BlockPos) {
        if !Self::can_stay_alive(state, world.as_ref(), pos) {
            world.set_block(
                pos,
                self.base_block.default_state(),
                UpdateFlags::UPDATE_ALL,
            );
            return;
        }

        if world.max_local_raw_brightness(pos.above(), 0) < 9 {
            return;
        }

        let default_state = self.block.default_state();
        let mut random = rand::rng();
        for _ in 0..4 {
            let candidate = pos.offset(
                random.random_range(0..3) - 1,
                random.random_range(0..5) - 3,
                random.random_range(0..3) - 1,
            );
            if world.get_block_state(candidate).get_block() != self.base_block
                || !Self::can_propagate(default_state, world.as_ref(), candidate)
            {
                continue;
            }

            let snowy = world
                .get_block_state(candidate.above())
                .get_block()
                .has_tag(&BlockTag::SNOW);
            let candidate_state = default_state.set_value(&BlockStateProperties::SNOWY, snowy);
            world.set_block(candidate, candidate_state, UpdateFlags::UPDATE_ALL);
        }
    }

    pub(super) fn can_stay_alive(
        state: BlockStateId,
        world: &dyn LevelReader,
        pos: BlockPos,
    ) -> bool {
        let above = pos.above();
        let above_state = world.get_block_state(above);
        if above_state.get_block() == &vanilla_blocks::SNOW
            && above_state.get_value(&BlockStateProperties::LAYERS) == 1
        {
            return true;
        }
        if above_state.get_fluid_state().is_full() {
            return false;
        }

        get_light_block_into(
            state,
            above_state,
            Direction::Up,
            above_state.get_light_dampening(),
        ) < 15
    }

    fn can_propagate(state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> bool {
        Self::can_stay_alive(state, world, pos)
            && !world
                .get_block_state(pos.above())
                .get_fluid_state()
                .is_water()
    }
}

#[cfg(test)]
mod tests {
    use steel_registry::blocks::block_state_ext::BlockStateExt;
    use steel_registry::blocks::properties::BlockStateProperties;
    use steel_registry::{init_vanilla_registry, vanilla_blocks};
    use steel_utils::BlockPos;

    use super::*;
    use crate::test_support::TestLevel;

    #[test]
    fn can_stay_alive_matches_snow_fluid_and_light_rules() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let grass = vanilla_blocks::GRASS_BLOCK.default_state();

        let open = TestLevel::default();
        assert!(SpreadingSnowyBlock::can_stay_alive(grass, &open, pos));

        let snow = vanilla_blocks::SNOW
            .default_state()
            .set_value(&BlockStateProperties::LAYERS, 1);
        let snow_level = TestLevel::default().with_block(pos.above(), snow);
        assert!(SpreadingSnowyBlock::can_stay_alive(grass, &snow_level, pos));

        let water_level =
            TestLevel::default().with_block(pos.above(), vanilla_blocks::WATER.default_state());
        assert!(!SpreadingSnowyBlock::can_stay_alive(
            grass,
            &water_level,
            pos
        ));

        let solid_level =
            TestLevel::default().with_block(pos.above(), vanilla_blocks::STONE.default_state());
        assert!(!SpreadingSnowyBlock::can_stay_alive(
            grass,
            &solid_level,
            pos
        ));
    }

    #[test]
    fn can_propagate_rejects_water_above_the_target() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let state = vanilla_blocks::GRASS_BLOCK.default_state();
        let level = TestLevel::default()
            .with_block(pos, vanilla_blocks::DIRT.default_state())
            .with_block(pos.above(), vanilla_blocks::WATER.default_state());

        assert!(!SpreadingSnowyBlock::can_propagate(state, &level, pos));
    }
}
