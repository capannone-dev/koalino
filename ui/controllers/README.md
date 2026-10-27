<!-- This directory contains Flutter state controllers that coordinate presentation and bridges. -->

# Controllers

Controllers translate asynchronous bridge outcomes into UI state. They depend on bridge interfaces
rather than generated Rust bindings, preserving the Flutter side of the hexagonal boundary.

The current controllers live in `lib/controllers/`, including `MeetingListController`
(Issue #36).
