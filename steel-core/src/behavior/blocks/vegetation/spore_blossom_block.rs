use steel_macros::block_behavior;
use steel_registry::blocks::block_state_ext::BlockStateExt as _;
use steel_registry::blocks::properties::Direction;
use steel_registry::blocks::shapes::SupportType;
use steel_registry::vanilla_block_tags::BlockTag;
use steel_registry::vanilla_blocks;
use steel_utils::{BlockPos, BlockStateId};

use crate::behavior::block::BlockBehavior;
use crate::behavior::context::BlockPlaceContext;
use crate::fluid::FluidStateExt as _;
use crate::world::{LevelReader, ScheduledTickAccess};

use super::{BlockRef, default_surviving_state};

/// Vanilla `SporeBlossomBlock` survival and support updates.
///
/// Vanilla's ambient spores come from client-local `animateTick`; there is no
/// server-side particle work for this block.
#[block_behavior]
pub struct SporeBlossomBlock {
    block: BlockRef,
}

impl SporeBlossomBlock {
    /// Creates a new spore blossom block behavior.
    #[must_use]
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }
}

impl BlockBehavior for SporeBlossomBlock {
    fn can_survive(&self, _state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> bool {
        let above_pos = pos.above();
        let above_state = world.get_block_state(above_pos);
        world.is_face_sturdy_for(above_state, above_pos, Direction::Down, SupportType::Center)
            && !above_state
                .get_block()
                .has_tag(&BlockTag::UNSTABLE_BOTTOM_CENTER)
            && !world.get_block_state(pos).get_fluid_state().is_water()
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        world: &dyn ScheduledTickAccess,
        pos: BlockPos,
        direction: Direction,
        _neighbor_pos: BlockPos,
        _neighbor_state: BlockStateId,
    ) -> BlockStateId {
        if direction == Direction::Up && !self.can_survive(state, world, pos) {
            vanilla_blocks::AIR.default_state()
        } else {
            state
        }
    }

    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        default_surviving_state(self.block, self, context)
    }
}

#[cfg(test)]
mod tests {
    use steel_registry::{init_vanilla_registry, vanilla_blocks};

    use super::*;
    use crate::test_support::TestLevel;

    fn behavior() -> SporeBlossomBlock {
        SporeBlossomBlock::new(&vanilla_blocks::SPORE_BLOSSOM)
    }

    struct CenterSupportLevel {
        above_state: BlockStateId,
    }

    impl LevelReader for CenterSupportLevel {
        fn get_block_state(&self, pos: BlockPos) -> BlockStateId {
            if pos.y() == 65 {
                self.above_state
            } else {
                vanilla_blocks::AIR.default_state()
            }
        }

        fn is_face_sturdy_for(
            &self,
            _state: BlockStateId,
            _pos: BlockPos,
            _direction: Direction,
            _support_type: SupportType,
        ) -> bool {
            true
        }

        fn raw_brightness(&self, _pos: BlockPos, _sky_darkening: u8) -> u8 {
            15
        }

        fn min_y(&self) -> i32 {
            -64
        }

        fn height(&self) -> i32 {
            384
        }
    }

    #[test]
    fn survives_with_sturdy_ceiling() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let level =
            TestLevel::default().with_block(pos.above(), vanilla_blocks::STONE.default_state());

        assert!(behavior().can_survive(vanilla_blocks::SPORE_BLOSSOM.default_state(), &level, pos));
    }

    #[test]
    fn does_not_survive_under_unstable_center_support() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let above_state = vanilla_blocks::OAK_FENCE_GATE.default_state();
        let level = CenterSupportLevel { above_state };

        assert!(
            above_state
                .get_block()
                .has_tag(&BlockTag::UNSTABLE_BOTTOM_CENTER)
        );
        assert!(!behavior().can_survive(
            vanilla_blocks::SPORE_BLOSSOM.default_state(),
            &level,
            pos
        ));
    }

    #[test]
    fn update_shape_removes_unsupported_block() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let state = vanilla_blocks::SPORE_BLOSSOM.default_state();

        assert_eq!(
            behavior().update_shape(
                state,
                &TestLevel::default(),
                pos,
                Direction::Up,
                pos.above(),
                vanilla_blocks::AIR.default_state(),
            ),
            vanilla_blocks::AIR.default_state()
        );
    }

    #[test]
    fn side_updates_do_not_remove_block() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let state = vanilla_blocks::SPORE_BLOSSOM.default_state();

        assert_eq!(
            behavior().update_shape(
                state,
                &TestLevel::default(),
                pos,
                Direction::North,
                pos.north(),
                vanilla_blocks::AIR.default_state(),
            ),
            state
        );
    }

    #[test]
    fn water_at_position_does_not_survive() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let level = TestLevel::default()
            .with_block(pos.above(), vanilla_blocks::STONE.default_state())
            .with_block(pos, vanilla_blocks::WATER.default_state());

        assert!(!behavior().can_survive(
            vanilla_blocks::SPORE_BLOSSOM.default_state(),
            &level,
            pos
        ));
    }

    #[test]
    fn lava_does_not_block_survival() {
        init_vanilla_registry();
        let pos = BlockPos::new(0, 64, 0);
        let level = TestLevel::default()
            .with_block(pos.above(), vanilla_blocks::STONE.default_state())
            .with_block(pos, vanilla_blocks::LAVA.default_state());

        assert!(behavior().can_survive(vanilla_blocks::SPORE_BLOSSOM.default_state(), &level, pos));
    }
}
