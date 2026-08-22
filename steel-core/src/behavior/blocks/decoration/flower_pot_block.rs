use std::sync::Arc;

use steel_macros::block_behavior;
use steel_registry::blocks::{BlockRef, block_state_ext::BlockStateExt};
use steel_registry::vanilla_blocks;
use steel_utils::types::UpdateFlags;
use steel_utils::{BlockPos, BlockStateId};

use crate::behavior::block::BlockBehavior;
use crate::behavior::context::BlockPlaceContext;
use crate::world::World;

use crate::behavior::blocks::vegetation::{EYEBLOSSOM_OPEN_ATTRIBUTE, EyeblossomType};

#[block_behavior]
/// Behavior for flower pots, including the two potted Eyeblossom variants.
pub struct FlowerPotBlock {
    block: BlockRef,
    #[json_arg(vanilla_blocks, json = "potted")]
    potted: BlockRef,
}

impl FlowerPotBlock {
    /// Creates a flower pot behavior for its contained block.
    #[must_use]
    pub const fn new(block: BlockRef, potted: BlockRef) -> Self {
        Self { block, potted }
    }
}

impl BlockBehavior for FlowerPotBlock {
    fn get_state_for_placement(&self, _context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        Some(self.block.default_state())
    }

    fn random_tick(&self, state: BlockStateId, world: &Arc<World>, pos: BlockPos) {
        if state.get_block() != self.block {
            return;
        }

        let current_type = if self.potted == &vanilla_blocks::OPEN_EYEBLOSSOM {
            EyeblossomType::Open
        } else if self.potted == &vanilla_blocks::CLOSED_EYEBLOSSOM {
            EyeblossomType::Closed
        } else {
            return;
        };

        let should_be_open =
            world.environment_bool_attribute(EYEBLOSSOM_OPEN_ATTRIBUTE, current_type.is_open());
        if should_be_open == current_type.is_open() {
            return;
        }

        let new_type = current_type.transform();
        let new_block = if new_type.is_open() {
            &vanilla_blocks::POTTED_OPEN_EYEBLOSSOM
        } else {
            &vanilla_blocks::POTTED_CLOSED_EYEBLOSSOM
        };
        if !world.set_block(pos, new_block.default_state(), UpdateFlags::UPDATE_ALL) {
            return;
        }

        new_type.spawn_transform_particle(world, pos);
        world.play_block_sound(new_type.long_switch_sound(), pos, 1.0, 1.0, None);
    }
}
