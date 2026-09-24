import { afterEach, describe, it, expect, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Button, EmptyState, ErrorState, Field, Input, Stack, Text } from "../../../frontend/web/src/ui/index";
import { color, space, text } from "../../../frontend/web/src/ui/theme";

// Isolated cases for the sample's web UI kit: interactive states, the form anatomy's aria wiring, typography, and the
// closed prop surface. Values are read from the theme so restyling the app never breaks the spec.

// Vitest runs without globals, so RTL can't register its auto-cleanup: do it explicitly or the DOM accumulates
// across cases and every query goes ambiguous.
afterEach(cleanup);

// jsdom normalizes some inline colors to rgb(); accept either spelling of the same color.
const rgb = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
};
const sameColor = (actual: string, hex: string) => expect([hex, rgb(hex)]).toContain(actual);

describe("Button", () => {
  it("FM-1: the label is the accessible name and a press fires the action", () => {
    const onClick = vi.fn();
    render(<Button label="Save" onClick={onClick} />);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("FM-2: a loading button blocks the action and announces itself", () => {
    const onClick = vi.fn();
    render(<Button label="Save" onClick={onClick} loading />);
    const btn = screen.getByRole("button") as HTMLButtonElement;
    fireEvent.click(btn);
    expect(onClick).toHaveBeenCalledTimes(0);
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("aria-busy")).toBe("true");
  });

  it("FM-2: a disabled button blocks the action", () => {
    const onClick = vi.fn();
    render(<Button label="Save" onClick={onClick} disabled />);
    fireEvent.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledTimes(0);
  });

  it("FM-3: focus shows the focus ring and blur drops it, never outline:none alone", () => {
    render(<Button label="Go" onClick={() => {}} />);
    const btn = screen.getByRole("button") as HTMLButtonElement;
    fireEvent.focus(btn);
    expect(btn.style.boxShadow).toBe("0 0 0 2px " + color.focusRing);
    fireEvent.blur(btn);
    expect(btn.style.boxShadow).toBe("none");
  });
});

describe("Field and Input, the form anatomy", () => {
  it("FM-4: the label is associated and describedby points at the hint", () => {
    render(
      <Field fieldId="email" label="Email" hint="We never share it">
        <Input id="email" value="" onChange={() => {}} kind="email" />
      </Field>,
    );
    const input = screen.getByLabelText("Email");
    expect(input.getAttribute("aria-describedby")).toBe("email-hint");
    expect(document.getElementById("email-hint")?.textContent).toBe("We never share it");
  });

  it("FM-4: an error replaces the hint as a role=alert and flips the control invalid", () => {
    render(
      <Field fieldId="email" label="Email" hint="We never share it" error="Required">
        <Input id="email" value="" onChange={() => {}} />
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

  it("FM-5: the input hands onChange the DOM change event carrying the typed value, and reports blur", () => {
    const typed: string[] = [];
    const onBlur = vi.fn();
    render(
      <Field fieldId="name" label="Name">
        <Input id="name" value="" onChange={(event) => typed.push(event.target.value)} onBlur={onBlur} />
      </Field>,
    );
    const input = screen.getByLabelText("Name");
    fireEvent.change(input, { target: { value: "Ada" } });
    fireEvent.blur(input);
    expect(typed).toEqual(["Ada"]);
    expect(onBlur).toHaveBeenCalledTimes(1);
  });
});

describe("Text, typography as one decision", () => {
  it("FM-6: a role maps to its type scale and the document outline", () => {
    render(<Text role="title">Hello</Text>);
    const el = screen.getByText("Hello");
    expect(el.tagName).toBe("H1");
    expect(el.style.fontSize).toBe(text.title.fontSize + "px");
    expect(el.style.fontWeight).toBe(String(text.title.fontWeight));
  });

  it("FM-6: a tone maps to its semantic color role", () => {
    expect.hasAssertions();
    render(<Text tone="muted">m</Text>);
    sameColor(screen.getByText("m").style.color, color.textMuted);
  });

  it("FM-6: the alert flag announces the text, the command-error surface", () => {
    render(
      <Text role="label" tone="danger" alert>
        Failed
      </Text>,
    );
    expect(screen.getByRole("alert").textContent).toBe("Failed");
  });
});

describe("Stack, rhythm from the scale", () => {
  it("FM-7: children are spaced by the gap scale and carry no margin", () => {
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

describe("Async states", () => {
  it("FM-8: the empty state renders its title and description", () => {
    render(<EmptyState title="Nothing yet" description="Create one" />);
    expect(screen.getByText("Nothing yet")).toBeTruthy();
    expect(screen.getByText("Create one")).toBeTruthy();
  });

  it("FM-8: the error state offers its retry action", () => {
    const onRetry = vi.fn();
    render(<ErrorState title="Boom" retryLabel="Try again" onRetry={onRetry} />);
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});

it("FM-9: the prop surface stays closed, with no className or style passthrough", () => {
  // Compile-time assertions, checked by `npm run typecheck`: styling passthrough is not part of the prop surface.
  // @ts-expect-error className is not part of the kit vocabulary
  const a = <Stack className="x">k</Stack>;
  // @ts-expect-error style is not part of the kit vocabulary
  const b = <Text style={{ padding: 13 }}>k</Text>;
  expect(a).toBeTruthy();
  expect(b).toBeTruthy();
});
