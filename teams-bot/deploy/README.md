<!-- Explains the deployment assets of the Teams bot. -->

# Deployment assets

- `Dockerfile`: a Windows Server Core container image of the bot host (`src/TeamsBot`). Build it
  from the repository root, because the build compiles `core/protocol/teams_bridge.proto`.
- `teams-app/`: the Teams app manifest template that declares calling and video support.

The main [README](../README.md#10-deployment) covers how to deploy locally, with Docker, or on
an Azure VM.
