// Entry point of the Teams bot host. It wires the pieces together:
//
//   Teams ──(Graph webhook /api/calling, media on TCP 8445)──▶ GraphCallJoiner / GraphCallSession
//        ──▶ BotService (AudioProcessor, VideoProcessor) ──▶ WebSocketClient ──▶ Rust app (127.0.0.1:8765)
//
// Configuration comes from appsettings.json, environment variables (AzureAd__ClientSecret,
// TeamsBot__SharedSecret, TeamsBot__ApiKey), and user secrets in development. Invalid settings
// stop the host at startup with a clear error rather than failing mid-meeting.

using Serilog;
using TeamsBot;
using TeamsBot.Core.Audio;
using TeamsBot.Core.Authentication;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Hosting;
using TeamsBot.Core.Meeting;
using TeamsBot.Media;

Log.Logger = new LoggerConfiguration().WriteTo.Console().CreateBootstrapLogger();
try
{
    var builder = WebApplication.CreateBuilder(args);
    builder.Host.UseSerilog((context, services, configuration) => configuration
        .ReadFrom.Configuration(context.Configuration)
        .ReadFrom.Services(services)
        .Enrich.FromLogContext());

    var botOptions = builder.Configuration.GetSection(TeamsBotOptions.SectionName).Get<TeamsBotOptions>() ?? new TeamsBotOptions();
    var azureAdOptions = builder.Configuration.GetSection(AzureAdOptions.SectionName).Get<AzureAdOptions>() ?? new AzureAdOptions();
    var callingOptions = builder.Configuration.GetSection(CallingOptions.SectionName).Get<CallingOptions>() ?? new CallingOptions();
    var configurationErrors = HostConfiguration.Validate(botOptions, azureAdOptions, callingOptions);
    if (configurationErrors.Count > 0)
    {
        foreach (var error in configurationErrors)
        {
            Log.Fatal("Configuration error: {Error}", error);
        }
        return 1;
    }

    builder.Services.AddSingleton(botOptions);
    builder.Services.AddSingleton(new ApiKeyVerifier(botOptions.ApiKey));
    builder.Services.AddSingleton(new MsalTokenSource(azureAdOptions));
    builder.Services.AddSingleton<IAccessTokenProvider>(services => new CachingAccessTokenProvider(
        services.GetRequiredService<MsalTokenSource>().AcquireAsync,
        services.GetRequiredService<ILogger<CachingAccessTokenProvider>>()));
    builder.Services.AddSingleton(services => new WebSocketClient(
        botOptions.ToWebSocketClientOptions(),
        services.GetRequiredService<ILogger<WebSocketClient>>()));
    builder.Services.AddSingleton(services => new GraphCallJoiner(
        azureAdOptions,
        callingOptions,
        services.GetRequiredService<IAccessTokenProvider>(),
        services.GetRequiredService<ILoggerFactory>()));
    builder.Services.AddSingleton(services => new BotService(
        services.GetRequiredService<GraphCallJoiner>(),
        services.GetRequiredService<WebSocketClient>(),
        new BotServiceOptions(),
        new AudioChunkingOptions(),
        botOptions.ToVideoProcessingOptions(),
        services.GetRequiredService<ILoggerFactory>()));
    builder.Services.AddHostedService<BotLifetimeService>();

    var app = builder.Build();
    app.UseSerilogRequestLogging();
    app.MapBotEndpoints();
    await app.RunAsync().ConfigureAwait(false);
    return 0;
}
catch (Exception failure) when (failure is not HostAbortedException)
{
    Log.Fatal(failure, "The Teams bot host stopped unexpectedly");
    return 1;
}
finally
{
    await Log.CloseAndFlushAsync().ConfigureAwait(false);
}
