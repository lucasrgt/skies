using System.Text.Json;

namespace Skies.Framework.Cli;

/// <summary>Runs the bounded foundation lifecycle owned by the primary coding agent.</summary>
internal static class FoundationWorkflowCommand
{
    internal sealed record Step(string Id, string Purpose, IReadOnlyList<string> Arguments);

    internal delegate int StepRunner(string id, string[] arguments);

    /// <summary>Recall governing repository context before work begins or its scope changes.</summary>
    internal static int Context(string[] arguments)
    {
        if (HelpRequested(arguments))
            return WriteContextUsage(Console.Out, false);
        if (!TryParse(arguments, allowGateMode: false, out var request, out var message))
            return Fail(Console.Error, "context", message!);
        return ExecuteContext(ContextPlan(request!), Console.Out, Console.Error);
    }

    /// <summary>Run deterministic proof and semantic foundation gates before work is accepted.</summary>
    internal static int Check(string[] arguments)
    {
        if (HelpRequested(arguments) || !TryParse(arguments, true, out var request, out _))
            return Check(arguments, RunStep, Console.Out, Console.Error);
        GateOptions.TryParse([.. CheckPlan(request!)[0].Arguments], out var options, out _);
        return GateAttempt.Run(Directory.GetCurrentDirectory(), options,
            () => Check(arguments, RunStep, Console.Out, Console.Error), Console.Error);
    }

    internal static int Context(
        string[] arguments,
        StepRunner runner,
        TextWriter output,
        TextWriter error)
    {
        if (HelpRequested(arguments))
            return WriteContextUsage(output, false);
        if (!TryParse(arguments, allowGateMode: false, out var request, out var message))
            return Fail(error, "context", message!);

        var steps = ContextPlan(request!);
        return Execute(
            "context",
            "governing context recalled",
            steps,
            runner,
            output);
    }

    internal static int Check(
        string[] arguments,
        StepRunner runner,
        TextWriter output,
        TextWriter error)
    {
        if (HelpRequested(arguments))
            return WriteCheckUsage(output, false);
        if (!TryParse(arguments, allowGateMode: true, out var request, out var message))
            return Fail(error, "check", message!);

        var steps = CheckPlan(request!);
        return Execute(
            "check",
            "all foundation gates passed",
            steps,
            runner,
            output);
    }

    internal static IReadOnlyList<Step> ContextPlan(Request request)
    {
        var common = TaskAndPaths(request);
        return
        [
            new("wtw", "governing decisions and invariants",
                ["explain", .. common, "--limit", request.Limit.ToString()]),
            new("rtw", "proven implementation ways",
                ["guide", .. common, "--limit", request.Limit.ToString()]),
            new("nya", "known corrected failures",
                ["recall", .. common, "--limit", request.Limit.ToString()]),
            new("nwc", "newly due conditional work", ["wake", .. Events(request)]),
        ];
    }

    internal static IReadOnlyList<Step> CheckPlan(Request request)
    {
        var gate = new List<string>();
        if (request.Full)
            gate.Add("--full");
        else if (request.Staged)
            gate.Add("--staged");
        else
            gate.Add("--affected");
        if (request.Fast || request.Staged)
            gate.Add("--fast");
        if (request.RetryReview is not null)
            gate.AddRange(["--retry-review", request.RetryReview]);
        if (request.Base is not null)
        {
            gate.Add("--base");
            gate.Add(request.Base);
        }

        var semantic = new List<string> { "--task", request.Task };
        if (request.Base is not null)
        {
            semantic.Add("--base");
            semantic.Add(request.Base);
        }

        var wtw = new List<string> { "guard" };
        wtw.AddRange(semantic);
        foreach (var path in request.Paths)
        {
            wtw.Add("--path");
            wtw.Add(path);
        }

        return
        [
            new("gate", "AVP proof closure", gate),
            new("wtw", "decision and invariant graph", wtw),
            new("rtw", "alignment with proven ways", ["check", .. semantic]),
            new("nya", "known failure recurrence review", ["check", .. semantic]),
            new("nwc", "due conditional work", ["check", .. Events(request)]),
        ];
    }

    internal sealed record Request(
        string Task,
        IReadOnlyList<string> Paths,
        IReadOnlyList<string> Events,
        string? Base,
        int Limit,
        bool Fast,
        bool Full,
        bool Staged,
        bool Affected)
    {
        internal string? RetryReview { get; init; }
    }

    internal static bool TryParse(
        IReadOnlyList<string> arguments,
        bool allowGateMode,
        out Request? request,
        out string? error)
    {
        string? task = null;
        string? baseRevision = null;
        var paths = new List<string>();
        var events = new List<string>();
        var limit = 8;
        var fast = false;
        var full = false;
        var staged = false;
        var affected = false;
        string? retryReview = null;

        for (var index = 0; index < arguments.Count; index++)
        {
            var argument = arguments[index];
            if (argument is "--task" or "--path" or "--event" or "--base" or "--limit" || allowGateMode && argument == "--retry-review")
            {
                if (++index >= arguments.Count || arguments[index].StartsWith("--", StringComparison.Ordinal))
                    return Invalid(out request, out error, $"{argument} requires a value");
                var value = arguments[index];
                switch (argument)
                {
                    case "--task": task = value; break;
                    case "--retry-review": retryReview = value; break;
                    case "--path": paths.Add(value); break;
                    case "--event": events.Add(value); break;
                    case "--base": baseRevision = value; break;
                    case "--limit" when int.TryParse(value, out var parsed) && parsed is >= 1 and <= 24:
                        limit = parsed;
                        break;
                    case "--limit":
                        return Invalid(out request, out error, "--limit must be between 1 and 24");
                }
                continue;
            }

            if (allowGateMode && argument is "--fast" or "--full" or "--staged" or "--affected")
            {
                fast |= argument == "--fast";
                full |= argument == "--full";
                staged |= argument == "--staged";
                affected |= argument == "--affected";
                continue;
            }

            return Invalid(out request, out error, $"unknown option {argument}");
        }

        if (string.IsNullOrWhiteSpace(task))
            return Invalid(out request, out error, "--task is required");
        if (!allowGateMode && baseRevision is not null)
            return Invalid(out request, out error, "--base is only valid for skies check");
        var scopes = (full ? 1 : 0) + (staged ? 1 : 0) + (affected ? 1 : 0) + (baseRevision is null ? 0 : 1);
        if (allowGateMode && scopes == 0)
        {
            return Invalid(
                out request,
                out error,
                "choose an explicit scope: --staged, --affected, --base <revision>, or --full");
        }
        if (scopes > 1)
            return Invalid(out request, out error, "check scopes are mutually exclusive");
        if (full && fast)
            return Invalid(out request, out error, "--full cannot be combined with --fast");

        request = new Request(task, paths, events, baseRevision, limit, fast, full, staged, affected) { RetryReview = retryReview };
        error = null;
        return true;
    }

    private static int Execute(
        string command,
        string success,
        IReadOnlyList<Step> steps,
        StepRunner runner,
        TextWriter output)
    {
        output.WriteLine($"skies {command}: primary-agent foundation workflow");
        var code = 0;
        foreach (var step in steps)
        {
            output.WriteLine();
            output.WriteLine($"[{step.Id}] {step.Purpose}");
            var result = runner(step.Id, [.. step.Arguments]);
            code = Math.Max(code, result);
            output.WriteLine(result == 0
                ? $"[{step.Id}] PASS"
                : $"[{step.Id}] {(result == 1 ? "FINDINGS" : "INCOMPLETE")} (exit {result})");
        }

        output.WriteLine();
        output.WriteLine(code == 0
            ? $"skies {command}: PASS, {success}."
            : $"skies {command}: BLOCKED. Stop and check whether you are in a loop. Diagnose the failure and validate a focused correction before repeating verification.");
        return code;
    }

    private static int ExecuteContext(
        IReadOnlyList<Step> steps,
        TextWriter output,
        TextWriter error)
    {
        output.WriteLine("skies context: primary-agent foundation workflow");
        var code = 0;
        foreach (var step in steps)
        {
            output.WriteLine();
            output.WriteLine($"[{step.Id}] {step.Purpose}");
            var arguments = MachineReadable(step);
            var result = CaptureStep(step.Id, arguments);
            if (!string.IsNullOrWhiteSpace(result.Error))
                error.Write(result.Error);
            if (!string.IsNullOrWhiteSpace(result.Output))
                WriteBounded(step.Id, result.Output, output);
            code = Math.Max(code, result.ExitCode);
            output.WriteLine(result.ExitCode == 0
                ? $"[{step.Id}] PASS"
                : $"[{step.Id}] {(result.ExitCode == 1 ? "FINDINGS" : "INCOMPLETE")} (exit {result.ExitCode})");
        }

        output.WriteLine();
        output.WriteLine(code == 0
            ? "skies context: PASS, governing context recalled."
            : "skies context: BLOCKED, context retrieval was incomplete.");
        return code;
    }

    private static string[] TaskAndPaths(Request request)
    {
        var result = new List<string> { "--task", request.Task };
        foreach (var path in request.Paths)
        {
            result.Add("--path");
            result.Add(path);
        }
        return [.. result];
    }

    private static string[] Events(Request request) =>
        request.Events.SelectMany(value => new[] { "--event", value }).ToArray();

    private static int RunStep(string id, string[] arguments) => id switch
    {
        "gate" => GateCommand.Run(arguments),
        "wtw" => WtwCommand.Run(arguments),
        "rtw" => RtwCommand.Run(arguments),
        "nya" => NyaCommand.Run(arguments),
        "nwc" => NwcCommand.Run(arguments),
        _ => 2,
    };

    private static FoundationTool.Execution CaptureStep(string id, string[] arguments) => id switch
    {
        "wtw" => WtwCommand.Capture(arguments),
        "rtw" => RtwCommand.Capture(arguments),
        "nya" => NyaCommand.Capture(arguments),
        "nwc" => NwcCommand.Capture(arguments),
        _ => new FoundationTool.Execution(2, "", $"skies context: unknown foundation {id}."),
    };

    private static string[] MachineReadable(Step step) => step.Id switch
    {
        "wtw" or "rtw" or "nwc" => [.. step.Arguments, "--json"],
        "nya" => [.. step.Arguments, "--format", "json"],
        _ => [.. step.Arguments],
    };

    internal static void WriteBounded(string id, string json, TextWriter output)
    {
        try
        {
            using var document = JsonDocument.Parse(json);
            switch (id)
            {
                case "wtw":
                    WriteWtw(document.RootElement, output);
                    return;
                case "rtw":
                    WriteArray(document.RootElement, "No relevant ways found.", "guidance", output);
                    return;
                case "nya":
                    WriteArray(document.RootElement, "No relevant scars found.", "lesson", output);
                    return;
                case "nwc":
                    WriteNwc(document.RootElement, output);
                    return;
            }
        }
        catch (JsonException)
        {
        }
        output.WriteLine(Compact(json));
    }

    private static void WriteWtw(JsonElement root, TextWriter output)
    {
        if (!root.TryGetProperty("records", out var records)
            || records.ValueKind != JsonValueKind.Array
            || records.GetArrayLength() == 0)
        {
            output.WriteLine("No governing decisions or invariants found.");
            return;
        }

        foreach (var record in records.EnumerateArray())
        {
            var kind = Text(record, "kind", "record");
            output.WriteLine($"  [{kind}] {Text(record, "title", Text(record, "id", "untitled"))}");
            output.WriteLine($"    {Compact(Text(record, "statement", ""))}");
            if (kind == "invariant" && record.TryGetProperty("violation", out var violation))
                output.WriteLine($"    violation: {Compact(violation.GetString() ?? "")}");
        }
    }

    private static void WriteArray(
        JsonElement root,
        string empty,
        string detail,
        TextWriter output)
    {
        if (root.ValueKind != JsonValueKind.Array || root.GetArrayLength() == 0)
        {
            output.WriteLine(empty);
            return;
        }

        foreach (var item in root.EnumerateArray())
        {
            output.WriteLine($"  [{Text(item, "id", "record")}] {Text(item, "title", "untitled")}");
            output.WriteLine($"    {Compact(Text(item, detail, ""))}");
            if (detail == "guidance" && item.TryGetProperty("references", out var references)
                && references.ValueKind == JsonValueKind.Array)
                foreach (var reference in references.EnumerateArray().Take(3))
                    output.WriteLine($"    reference: {Compact(reference.ToString())}");
        }
    }

    private static void WriteNwc(JsonElement root, TextWriter output)
    {
        if (!root.TryGetProperty("due", out var due)
            || due.ValueKind != JsonValueKind.Array
            || due.GetArrayLength() == 0)
        {
            output.WriteLine("No deferments are due.");
            return;
        }

        foreach (var item in due.EnumerateArray())
        {
            output.WriteLine($"  [{Text(item, "id", "deferment")}] {Text(item, "title", "due work")}");
            output.WriteLine($"    {Compact(Text(item, "action", Text(item, "statement", "")))}");
        }
    }

    private static string Text(JsonElement element, string property, string fallback) =>
        element.TryGetProperty(property, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? fallback
            : fallback;

    private static string Compact(string value)
    {
        var compact = string.Join(' ', value
            .Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries));
        return compact.Length <= 480 ? compact : compact[..477] + "...";
    }

    private static bool HelpRequested(IReadOnlyCollection<string> arguments) =>
        arguments.Any(argument => argument is "--help" or "-h" or "-?");

    private static bool Invalid(
        out Request? request,
        out string? error,
        string message)
    {
        request = null;
        error = message;
        return false;
    }

    private static int Fail(TextWriter error, string command, string message)
    {
        error.WriteLine($"skies {command}: {message}.");
        error.WriteLine($"Run `skies {command} --help` for usage.");
        return 2;
    }

    private static int WriteContextUsage(TextWriter writer, bool error)
    {
        writer.WriteLine(
            """
            usage:
              skies context --task <goal> [--path <expected-path>]... [--event <observed-event>]... [--limit <1-24>]

            Recalls bounded WTW decisions, RTW ways, NYA scars, and NWC obligations
            for the primary coding agent. Run at task start and after scope or context changes.
            """);
        return error ? 2 : 0;
    }

    private static int WriteCheckUsage(TextWriter writer, bool error)
    {
        writer.WriteLine(
            """
            usage:
              skies check --task <staged-work> --staged
              skies check --task <working-tree> --affected [--fast]
              skies check --task <review> --base <revision> [--fast]
              skies check --task <release> --full
              Add --retry-review <json-file> after a failed or interrupted expensive attempt.

            Runs the AVP gate plus WTW, RTW, NYA, and NWC checks as one fail-closed
            receipt. Staged checks are always bounded and defer exhaustive/browser fallbacks
            to the repository's selected authority boundary. Exit 1 means findings; exit 2 or greater means
            incomplete validation.
            """);
        return error ? 2 : 0;
    }
}
