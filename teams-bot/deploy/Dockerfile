# escape=`
# Windows container image of the Teams bot. The Real-Time Media platform needs Windows Server
# (Server Core, not Nano Server), so build and run this on a Windows container host:
#   docker build -f teams-bot/deploy/Dockerfile -t note-taker-teams-bot .     (from the repo root)
# The repository root is the build context because the bot compiles core/protocol/teams_bridge.proto.

FROM mcr.microsoft.com/dotnet/sdk:8.0-windowsservercore-ltsc2022 AS build
WORKDIR C:\src
COPY core\protocol core\protocol
COPY teams-bot teams-bot
RUN dotnet publish teams-bot\src\TeamsBot\TeamsBot.csproj --configuration Release --output C:\app

FROM mcr.microsoft.com/dotnet/aspnet:8.0-windowsservercore-ltsc2022
WORKDIR C:\app
COPY --from=build C:\app .
# 9441: HTTP signaling (/api/calling, /api/join) behind a TLS reverse proxy; 8445: media (TCP).
EXPOSE 9441 8445
ENV ASPNETCORE_URLS=http://+:9441
ENTRYPOINT ["dotnet", "TeamsBot.dll"]
