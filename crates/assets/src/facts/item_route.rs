//! Items whose identifier differs from the block they place, from the pinned route table.

use std::{collections::HashMap, sync::OnceLock};

const SOURCE: &str = include_str!("../../data/block-item-routes-v2225.json");

#[derive(serde::Deserialize)]
struct Table {
    routes: Vec<Route>,
}

#[derive(serde::Deserialize)]
struct Route {
    identifier: String,
    block_name: String,
}

/// The block an item places when the route table names one other than the item itself, such
/// as `minecraft:oak_sign` placing `minecraft:standing_sign`. Items whose route names their
/// own identifier, and items without a route, answer `None`.
pub fn item_block_route(identifier: &str) -> Option<&'static str> {
    static ROUTES: OnceLock<HashMap<String, String>> = OnceLock::new();
    ROUTES
        .get_or_init(|| {
            let table: Table = serde_json::from_str(SOURCE).expect("valid block item routes");
            table
                .routes
                .into_iter()
                .filter(|route| route.identifier != route.block_name)
                .map(|route| (route.identifier, route.block_name))
                .collect()
        })
        .get(identifier)
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::item_block_route;

    #[test]
    fn sign_and_banner_items_route_to_their_standing_blocks() {
        assert_eq!(
            item_block_route("minecraft:oak_sign"),
            Some("minecraft:standing_sign")
        );
        assert_eq!(
            item_block_route("minecraft:dark_oak_sign"),
            Some("minecraft:darkoak_standing_sign")
        );
        assert_eq!(
            item_block_route("minecraft:banner"),
            Some("minecraft:standing_banner")
        );
        assert_eq!(item_block_route("minecraft:stone"), None);
    }
}
