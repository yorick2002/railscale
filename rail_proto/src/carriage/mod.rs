pub use rail_carriage::door;
pub use rail_carriage::manifest;
pub use rail_carriage::gate;
pub use rail_carriage::disembark;
pub use rail_carriage::ticket_pipeline;
pub use rail_carriage::passengers;

#[cfg(not(feature = "tailscale"))]
pub mod nontailscale;
