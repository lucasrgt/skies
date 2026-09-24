import { afterEach, describe, it, expect, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Button, EmptyState, ErrorState, Field, Input, Stack, Text } from "./index";
import { color, space, text } from "./theme";

// Component tests for the sample's UI kit: interactive states, the form anatomy's aria wiring, typography, and the
// closed prop surface. Values are read from ./theme so restyling the app never breaks the suite.

// Vitest runs without globals, so RTL can't register its auto-cleanup — do it explicitly or the
// DOM accumulates across tests and every query goes ambiguous.
afterEach(cleanup);

// jsdom normalizes some inline colors to rgb(); accept either spelling of the same color.
const rgb = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
};
const sameColor = (actual: string, hex: string) => expect([hex, rgb(hex)]).toContain(actual);

describe("Button", () => {
  it("renders its label as the accessible name and fires onPress", () => {
    const onPress = vi.fn();
    render(<Button label="Save" onPress={onPress} />);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it("blocks the action and announces while loading", () => {
    const onPress = vi.fn();
    render(<Button label="Save" onPress={onPress} loading />);
    const btn = screen.getByRole("button") as HTMLButtonElement;
    fireEvent.click(btn);
    expect(onPress).toHaveBeenCalledTimes(0);
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("aria-busy")).toBe("true");
  });

  it("blocks when disabled", () => {
    const onPress = vi.fn();
    render(<Button label="Save" onPress={onPress} disabled />);
    fireEvent.click(screen.getByRole("button"));
    expect(onPress).toHaveBeenCalledTimes(0);
  });

  it("shows the focusRing treatment on focus and drops it on blur — never outline:none alone", () => {
    render(<Button label="Go" onPress={() => {}} />);
    const btn = screen.getByRole("button") as HTMLButtonElement;
    fireEvent.focus(btn);
    expect(btn.style.boxShadow).toBe("0 0 0 2px " + color.focusRing);
    fireEvent.blur(btn);
    expect(btn.style.boxShadow).toBe("none");
  });
});

describe("Field + Input — the form anatomy", () => {
  it("associates the label and points describedby at the hint", () => {
    render(
      <Field fieldId="email" label="Email" hint="We never share it">
        <Input id="email" value="" onChangeText={() => {}} kind="email" />
      </Field>,
    );
    const input = screen.getByLabelText("Email");
    expect(input.getAttribute("aria-describedby")).toBe("email-hint");
    expect(document.getElementById("email-hint")?.textContent).toBe("We never share it");
  });

  it("replaces the hint with a role=alert error and flips the control invalid", () => {
    render(
      <Field fieldId="email" label="Email" hint="We never share it" error="Required">
        <Input id="email" value="" onChangeText={() => {}} />
      </Field>,
    );
    expect(screen.queryByText("We never share it")).toBeNull();
    const alert = screen.getByRole("alert");
    expect(alert.id).toBe("email-error");
    expect(alert.textContent).toBe("Required");
    const input = screen.getByLabelText("Email") as HTMLInputElement;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe("email-error");
    sameColor(input.style.borderColor, color.danger);
  });

  it("hands the View a string through onChangeText, not an event", () => {
    const onChangeText = vi.fn();
    render(
      <Field fieldId="name" label="Name">
        <Input id="name" value="" onChangeText={onChangeText} />
      </Field>,
    );
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Ada" } });
    expect(onChangeText).toHaveBeenCalledWith("Ada");
  });
});

describe("Text — typography is one decision", () => {
  it("maps a role to its type scale and the document outline", () => {
    render(<Text role="title">Hello</Text>);
    const el = screen.getByText("Hello");
    expect(el.tagName).toBe("H1");
    expect(el.style.fontSize).toBe(text.title.fontSize + "px");
    expect(el.style.fontWeight).toBe(String(text.title.fontWeight));
  });

  it("maps tones to semantic color roles", () => {
    expect.hasAssertions();
    render(<Text tone="muted">m</Text>);
    sameColor(screen.getByText("m").style.color, color.textMuted);
  });

  it("announces as an alert when asked — the command-error surface", () => {
    render(
      <Text role="label" tone="danger" alert>
        Failed
      </Text>,
    );
    expect(screen.getByRole("alert").textContent).toBe("Failed");
  });
});

describe("Stack — rhythm from the scale", () => {
  it("spaces children with the gap scale; children carry no margin", () => {
    render(
      <Stack gap="lg">
        <Text>a</Text>
        <Text>b</Text>
      </Stack>,
    );
    const stack = screen.getByText("a").parentElement as HTMLElement;
    expect(stack.style.gap).toBe(space.lg + "px");
    expect(screen.getByText("a").style.margin).toBe("0px");
  });
});

describe("states", () => {
  it("EmptyState renders title and description", () => {
    render(<EmptyState title="Nothing yet" description="Create one" />);
    expect(screen.getByText("Nothing yet")).toBeTruthy();
    expect(screen.getByText("Create one")).toBeTruthy();
  });

  it("ErrorState offers the retry action when given one", () => {
    const onRetry = vi.fn();
    render(<ErrorState title="Boom" retryLabel="Try again" onRetry={onRetry} />);
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});

it("keeps the API closed — no className/style passthrough anywhere", () => {
  // Compile-time assertions: styling passthrough is not part of the prop surface.
  // @ts-expect-error className is not part of the kit vocabulary
  const a = <Stack className="x">k</Stack>;
  // @ts-expect-error style is not part of the kit vocabulary
  const b = <Text style={{ padding: 13 }}>k</Text>;
  expect(a).toBeTruthy();
  expect(b).toBeTruthy();
});
