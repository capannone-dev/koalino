<!-- Documents continuous-integration workflows maintained by this repository. -->

# Workflows

`web-ci.yml` verifies the standalone Next.js dashboard with a pinned Node release on relevant
pushes and pull requests.

`flutter-ci.yml` runs `make flutter-lint` (`dart format --set-exit-if-changed` plus
`flutter analyze`) and `make flutter-test` on the latest stable Flutter for pushes and pull
requests that touch `ui/`, the `Makefile`, or the workflow itself. The widget tests use fake
bridges, so no native Rust library is built or loaded.
