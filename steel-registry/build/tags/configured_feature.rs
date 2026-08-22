use proc_macro2::TokenStream;

use super::common;

pub(crate) fn build() -> TokenStream {
    common::build_simple_tags_with_tag_name(
        "worldgen/configured_feature",
        "feature",
        "ConfiguredFeatureRegistry",
        "configured_feature",
    )
}
