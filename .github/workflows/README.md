<!-- Documents continuous-integration workflows maintained by this repository. -->

# Workflows

`web-ci.yml` verifies the standalone Next.js dashboard with Node 22 on relevant pushes and pull
requests: `npm install` (with the npm cache keyed on `web/package.json`, because
`package-lock.json` is gitignored), `npm run lint`, the Vitest component and
API client tests with the verbose reporter (so each test is listed in the log), and
`npm run build`. A newer push to the same branch cancels an older run.

`flutter-ci.yml` runs `make flutter-lint` (`dart format --set-exit-if-changed` plus
`flutter analyze`) and `make flutter-test` on pinned Flutter 3.47.8 for pushes and pull
requests that touch `ui/`, the `Makefile`, or the workflow itself. The widget tests use fake
bridges, so no native Rust library is built or loaded.

`teams-bot-ci.yml` builds the C# Teams bot (`teams-bot/TeamsBot.sln`, warnings as errors) and
runs `dotnet test` on ubuntu-latest and windows-latest for pushes and pull requests that touch
`teams-bot/`, the shared `core/protocol/` schema and fixtures, or the workflow itself. The tests
mock the Teams SDK, so no Azure resources or media platform are needed.
