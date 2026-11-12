<!-- This directory is the Rust hexagonal core. Domain rules are isolated from ports, application orchestration, and adapters. -->

# Rust core

`domain` contains pure business types and errors. `ports` declares dependency-inversion interfaces. `application` coordinates use cases exclusively through ports. `adapters` contains infrastructure implementations. `server` translates injected ports into the headless Axum REST/WebSocket API. `bin` provides executable composition roots.

`assets` holds data compiled into the core: the public speech clip the on-device speed check times
(#109). `lib.rs` declares the modules. `cpu_stopwatch_for_tests.rs` is compiled only for tests: speed checks time their work with it, by the thread's CPU time, so a busy machine does not fail them (#81).
