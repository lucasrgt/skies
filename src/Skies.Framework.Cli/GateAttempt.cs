using System.Diagnostics;
using System.Text.Json;

namespace Skies.Framework.Cli;

/// <summary>Stops unattended expensive retries and overlapping verification in one checkout.</summary>
internal sealed class GateAttempt : IDisposable
{
    private static readonly AsyncLocal<bool> Active = new();

    internal static int Run(string root, GateOptions options, Func<int> action, TextWriter error)
    {
        if (options.Fast || options.Mode == GateMode.Staged || Active.Value)
            return action();
        try
        {
            using var attempt = Begin(DirectoryFor(root), options.Mode.ToString(), options.RetryReview, error);
            if (attempt is null)
                return 2;
            Console.WriteLine($"skies gate: attempt {attempt.receipt.Id}; receipt: {attempt.receiptPath}");
            Active.Value = true;
            try
            {
                var code = action();
                attempt.Complete(code);
                if (code != 0)
                    error.WriteLine("STOP — check whether you are in a loop. The automatic verification attempt is spent. Diagnose and verify a focused correction before retrying.");
                return code;
            }
            finally { Active.Value = false; }
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or System.ComponentModel.Win32Exception)
        {
            error.WriteLine($"skies gate: attempt tracking is unavailable; verification is incomplete: {exception.Message}");
            return 2;
        }
    }

    private readonly FileStream lease;
    private readonly string receiptPath;
    private readonly Receipt receipt;

    private GateAttempt(FileStream lease, string receiptPath, Receipt receipt)
    {
        this.lease = lease;
        this.receiptPath = receiptPath;
        this.receipt = receipt;
    }

    internal sealed record Receipt(string Id, string Status, string Scope, DateTimeOffset Started, string? Review);
    internal sealed record Review(string PreviousAttemptId, string Diagnosis, string Correction, string FocusedVerification);

    internal static GateAttempt? Begin(string directory, string scope, string? reviewPath, TextWriter error)
    {
        FileStream? lease = null;
        try
        {
            Directory.CreateDirectory(directory);
            lease = new FileStream(Path.Combine(directory, "active.lock"), FileMode.OpenOrCreate,
                FileAccess.ReadWrite, FileShare.None);
            var path = Path.Combine(directory, "attempt.json");
            var previous = File.Exists(path) ? JsonSerializer.Deserialize<Receipt>(File.ReadAllText(path))
                ?? throw new InvalidDataException("Empty attempt receipt.") : null;
            if (previous is not null && (string.IsNullOrWhiteSpace(previous.Id) || string.IsNullOrWhiteSpace(previous.Status)))
                throw new InvalidDataException("Malformed attempt receipt.");
            string? review = null;
            if (previous is not null && previous.Status != "passed")
            {
                if (reviewPath is null)
                    throw new InvalidDataException($"Attempt {previous.Id} is {previous.Status}. One automatic attempt is allowed.");
                review = File.ReadAllText(reviewPath);
                var audit = JsonSerializer.Deserialize<Review>(review);
                if (audit is null || audit.PreviousAttemptId != previous.Id
                    || string.IsNullOrWhiteSpace(audit.Diagnosis) || string.IsNullOrWhiteSpace(audit.Correction)
                    || string.IsNullOrWhiteSpace(audit.FocusedVerification))
                    throw new InvalidDataException("Retry review must name the previous attempt and include Diagnosis, Correction and FocusedVerification.");
            }
            var receipt = new Receipt(Guid.NewGuid().ToString("N"), "interrupted-or-running", scope, DateTimeOffset.UtcNow, review);
            File.WriteAllText(path, JsonSerializer.Serialize(receipt));
            return new GateAttempt(lease, path, receipt);
        }
        catch (Exception exception) when (exception is IOException or InvalidDataException or UnauthorizedAccessException or JsonException)
        {
            lease?.Dispose();
            error.WriteLine($"skies gate: STOP — check whether you are in a loop. {exception.Message}");
            error.WriteLine("Do not restart the broad check or launch another in parallel. Inspect the first failure, fix its cause, "
                + "and run focused verification. Supply --retry-review <json-file> with PreviousAttemptId, Diagnosis, Correction, "
                + "and FocusedVerification before one further attempt. This review records evidence; it does not replace any gate.");
            return null;
        }
    }

    internal static string DirectoryFor(string root)
    {
        if (!Directory.Exists(Path.Combine(root, ".git")) && !File.Exists(Path.Combine(root, ".git")))
            return Path.Combine(root, ".skies", "verification-attempt");
        var info = new ProcessStartInfo("git")
        {
            WorkingDirectory = root, UseShellExecute = false,
            RedirectStandardOutput = true, RedirectStandardError = true,
        };
        foreach (var argument in new[] { "rev-parse", "--absolute-git-dir" })
            info.ArgumentList.Add(argument);
        using var process = Process.Start(info) ?? throw new IOException("Could not locate Git metadata.");
        var output = process.StandardOutput.ReadToEndAsync();
        var error = process.StandardError.ReadToEndAsync();
        process.WaitForExit();
        if (process.ExitCode != 0)
            throw new IOException(error.GetAwaiter().GetResult());
        return Path.Combine(output.GetAwaiter().GetResult().Trim(), "skies-verification");
    }

    internal void Complete(int exitCode) => File.WriteAllText(receiptPath,
        JsonSerializer.Serialize(receipt with { Status = exitCode == 0 ? "passed" : $"failed-exit-{exitCode}" }));

    public void Dispose() => lease.Dispose();
}
