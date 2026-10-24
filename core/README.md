<!-- This directory is the Rust hexagonal core. Domain rules are isolated from ports, application orchestration, and adapters. -->

# Rust core

`domain` contains pure business types and errors. `ports` declares dependency-inversion interfaces. `application` coordinates use cases exclusively through ports. `adapters` contains infrastructure implementations. `bin` provides executable composition roots.
