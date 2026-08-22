use std::sync::Arc;

use rand::{Rng, RngExt};
use steel_macros::block_behavior;
use steel_registry::biome::Biome;
use steel_registry::blocks::{
    BlockRef, block_state_ext::BlockStateExt, shapes::is_shape_full_block,
};
use steel_registry::feature::{ConfiguredFeature, ConfiguredFeatureKind, ConfiguredFeatureRef};
use steel_registry::{REGISTRY, vanilla_blocks, vanilla_placed_features};
use steel_utils::random::worldgen_random::WorldgenRandom;
use steel_utils::{BlockPos, BlockStateId, Direction};

use super::bonemealable::{BonemealAction, Bonemealable};
use super::spreading_snowy_block::SpreadingSnowyBlock;
use crate::behavior::context::BlockPlaceContext;
use crate::behavior::{BLOCK_BEHAVIORS, BlockBehavior, BlockCollisionContext};
use crate::world::{LevelReader, ScheduledTickAccess, World};
use crate::worldgen::feature::FeatureDecorationRunner;

/// Behavior for grass blocks.
#[block_behavior]
pub struct GrassBlock {
    spreading: SpreadingSnowyBlock,
}

impl GrassBlock {
    /// Creates a new grass block behavior.
    #[must_use]
    pub const fn new(block: BlockRef) -> Self {
        Self {
            spreading: SpreadingSnowyBlock::new(block, &vanilla_blocks::DIRT),
        }
    }
}

impl BlockBehavior for GrassBlock {
    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        Some(self.spreading.get_state_for_placement(context))
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        _world: &dyn ScheduledTickAccess,
        _pos: BlockPos,
        direction: Direction,
        _neighbor_pos: BlockPos,
        neighbor_state: BlockStateId,
    ) -> BlockStateId {
        SpreadingSnowyBlock::update_shape(state, direction, neighbor_state)
    }

    fn random_tick(&self, state: BlockStateId, world: &Arc<World>, pos: BlockPos) {
        self.spreading.random_tick(state, world, pos);
    }

    fn as_bonemealable(&self) -> Option<&dyn Bonemealable> {
        Some(self)
    }
}

impl Bonemealable for GrassBlock {
    fn is_valid_bonemeal_target(
        &self,
        _state: BlockStateId,
        world: &dyn LevelReader,
        pos: BlockPos,
    ) -> bool {
        let above = pos.above();
        world.get_block_state(above).is_air() && !world.is_outside_build_height(above.y())
    }

    fn perform_bonemeal(
        &self,
        _state: BlockStateId,
        world: &Arc<World>,
        rng: &mut dyn Rng,
        pos: BlockPos,
    ) {
        let above = pos.above();
        let grass = vanilla_blocks::SHORT_GRASS.default_state();
        let mut feature_random = WorldgenRandom::from_seed(rng.random());

        'attempt: for j in 0..128 {
            let mut test_pos = above;
            for _ in 0..j / 16 {
                test_pos = test_pos.offset(
                    rng.random_range(0..3) - 1,
                    (rng.random_range(0..3) - 1) * rng.random_range(0..3) / 2,
                    rng.random_range(0..3) - 1,
                );
                let test_state = world.get_block_state(test_pos);
                if !self
                    .spreading
                    .is_block(world.get_block_state(test_pos.below()).get_block())
                    || is_collision_shape_full_block(world, test_state, test_pos)
                {
                    continue 'attempt;
                }
            }

            let test_state = world.get_block_state(test_pos);
            if test_state.get_block() == grass.get_block() && rng.random_range(0..10) == 0 {
                let behavior = BLOCK_BEHAVIORS.get_behavior(test_state.get_block());
                if let Some(bonemealable) = behavior.as_bonemealable()
                    && bonemealable.is_valid_bonemeal_target(test_state, world.as_ref(), test_pos)
                {
                    bonemealable.perform_bonemeal(test_state, world, rng, test_pos);
                }
            }

            if !test_state.is_air() || world.is_outside_build_height(test_pos.y()) {
                continue;
            }

            if rng.random_range(0..8) == 0 {
                let features = world
                    .biome_at(test_pos)
                    .map(Biome::bone_meal_features)
                    .unwrap_or_default();
                if !features.is_empty() {
                    let feature = features[rng.random_range(0..features.len())];
                    place_simple_configured_feature(world, feature, &mut feature_random, test_pos);
                }
            } else {
                place_grass_feature(world, &mut feature_random, test_pos);
            }
        }
    }

    fn bonemeal_action_type(&self) -> BonemealAction {
        BonemealAction::NeighborSpreader
    }
}

fn is_collision_shape_full_block(world: &Arc<World>, state: BlockStateId, pos: BlockPos) -> bool {
    let shape = BLOCK_BEHAVIORS
        .get_behavior(state.get_block())
        .get_collision_shape(state, world.as_ref(), pos, BlockCollisionContext::empty());
    is_shape_full_block(shape)
}

fn place_simple_configured_feature(
    world: &Arc<World>,
    feature: &'static ConfiguredFeature,
    random: &mut WorldgenRandom,
    pos: BlockPos,
) -> bool {
    let ConfiguredFeatureKind::SimpleBlock(config) = &feature.kind else {
        return false;
    };
    FeatureDecorationRunner::place_simple_block_feature_live(world, &REGISTRY, random, config, pos)
}

fn place_grass_feature(world: &Arc<World>, random: &mut WorldgenRandom, pos: BlockPos) -> bool {
    let placed_feature = &*vanilla_placed_features::GRASS_BONEMEAL;
    let ConfiguredFeatureRef::Reference(feature) = &placed_feature.data.feature else {
        return false;
    };
    place_simple_configured_feature(world, feature, random, pos)
}

#[cfg(test)]
mod tests {
    use steel_registry::blocks::block_state_ext::BlockStateExt;
    use steel_registry::blocks::properties::BlockStateProperties;
    use steel_registry::{init_vanilla_registry, vanilla_blocks};
    use steel_utils::{BlockPos, ChunkPos, Direction, types::UpdateFlags};

    use super::*;
    use crate::behavior::init_behaviors;
    use crate::test_support::{TestLevel, fresh_test_world, insert_ready_full_chunk};

    #[test]
    fn grass_block_updates_snowy_state() {
        init_vanilla_registry();
        init_behaviors();

        let level = TestLevel::default();
        let pos = BlockPos::new(0, 64, 0);
        let behavior = GrassBlock::new(&vanilla_blocks::GRASS_BLOCK);

        let non_snowy = vanilla_blocks::GRASS_BLOCK.default_state();
        let snowy = behavior.update_shape(
            non_snowy,
            &level,
            pos,
            Direction::Up,
            pos.above(),
            vanilla_blocks::SNOW.default_state(),
        );
        assert!(snowy.get_value(&BlockStateProperties::SNOWY));

        let cleared = behavior.update_shape(
            snowy,
            &level,
            pos,
            Direction::Up,
            pos.above(),
            vanilla_blocks::AIR.default_state(),
        );
        assert!(!cleared.get_value(&BlockStateProperties::SNOWY));
    }

    #[test]
    fn grass_block_turns_to_dirt_when_covered() {
        init_vanilla_registry();
        init_behaviors();

        let world = fresh_test_world("grass_block_turns_to_dirt");
        insert_ready_full_chunk(&world, ChunkPos::new(0, 0));
        let pos = BlockPos::new(0, 64, 0);
        assert!(world.set_block(
            pos,
            vanilla_blocks::GRASS_BLOCK.default_state(),
            UpdateFlags::UPDATE_NONE,
        ));
        assert!(world.set_block(
            pos.above(),
            vanilla_blocks::STONE.default_state(),
            UpdateFlags::UPDATE_NONE,
        ));

        GrassBlock::new(&vanilla_blocks::GRASS_BLOCK).random_tick(
            vanilla_blocks::GRASS_BLOCK.default_state(),
            &world,
            pos,
        );

        assert_eq!(
            world.get_block_state(pos).get_block(),
            &vanilla_blocks::DIRT
        );
    }

    #[test]
    fn grass_block_bonemeal_requires_air_inside_build_height() {
        init_vanilla_registry();
        let behavior = GrassBlock::new(&vanilla_blocks::GRASS_BLOCK);
        let pos = BlockPos::new(0, 64, 0);
        let state = vanilla_blocks::GRASS_BLOCK.default_state();

        assert!(behavior.is_valid_bonemeal_target(state, &TestLevel::default(), pos));
        assert!(!behavior.is_valid_bonemeal_target(
            state,
            &TestLevel::default().with_block(pos.above(), vanilla_blocks::STONE.default_state()),
            pos,
        ));
        assert!(!behavior.is_valid_bonemeal_target(
            state,
            &TestLevel::default().with_min_y(0),
            BlockPos::new(0, 383, 0),
        ));
    }
}
