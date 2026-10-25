<!-- This directory contains Flutter state controllers that coordinate presentation and bridges. -->

# Controllers

Controllers translate asynchronous bridge outcomes into UI state. They depend on bridge interfaces
rather than generated Rust bindings, preserving the Flutter side of the hexagonal boundary.
