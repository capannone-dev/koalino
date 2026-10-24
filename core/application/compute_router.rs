//! Defines a policy-only seam for selecting local or decentralized remote AI execution.

use crate::domain::models::DeviceId;

/// Captures the user-selected policy for an AI workload without selecting an adapter directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputePreference {
    /// Requires processing to run on the capturing device.
    LocalOnly,
    /// Requires processing on a user-selected, authenticated peer.
    SpecificPeer(DeviceId),
    /// Permits a future policy implementation to select the best capable peer.
    Automatic,
}
