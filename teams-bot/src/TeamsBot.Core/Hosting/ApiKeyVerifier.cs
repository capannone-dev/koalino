// Constant-time check of the X-Api-Key header that protects the bot's control endpoint
// (POST /api/join), which is reachable from the internet alongside the calling webhook.

using System.Security.Cryptography;
using System.Text;

namespace TeamsBot.Core.Hosting;

/// <summary>Compares presented API keys against the configured one without timing leaks.</summary>
public sealed class ApiKeyVerifier
{
    /// <summary>Configured keys shorter than this disable the endpoint.</summary>
    public const int MinimumKeyLength = 32;

    /// <summary>SHA-256 of the configured key, or <c>null</c> when disabled.</summary>
    private readonly byte[]? expectedHash;

    /// <summary>Creates the verifier.</summary>
    /// <param name="configuredKey">The key from configuration (may be missing).</param>
    public ApiKeyVerifier(string? configuredKey)
    {
        if (configuredKey is { Length: >= MinimumKeyLength })
        {
            expectedHash = SHA256.HashData(Encoding.UTF8.GetBytes(configuredKey));
        }
    }

    /// <summary>Whether a strong enough key is configured.</summary>
    public bool IsEnabled => expectedHash is not null;

    /// <summary>Checks a presented key.</summary>
    /// <param name="providedKey">The header value (may be missing).</param>
    /// <returns><c>true</c> only when enabled and the key matches exactly.</returns>
    public bool IsAuthorized(string? providedKey)
    {
        if (expectedHash is null || string.IsNullOrEmpty(providedKey))
        {
            return false;
        }
        // Hashing first makes the comparison length-independent.
        return CryptographicOperations.FixedTimeEquals(expectedHash, SHA256.HashData(Encoding.UTF8.GetBytes(providedKey)));
    }
}
