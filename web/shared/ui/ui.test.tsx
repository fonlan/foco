import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import {
  Button,
  Description,
  FieldError,
  Input,
  Label,
  Spinner,
  Switch,
  TextField,
  iconButton,
} from "./index";

describe("shared/ui HeroUI barrel", () => {
  it("exposes accessible name, pending, disabled, and danger button states", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();

    render(
      <div>
        <Button onPress={onPress}>Save changes</Button>
        <Button isDisabled>Disabled action</Button>
        <Button isPending>
          {({ isPending }) => (
            <>
              {isPending ? <Spinner color="current" size="sm" /> : null}
              Pending action
            </>
          )}
        </Button>
        <Button variant="danger" onPress={onPress}>
          Delete item
        </Button>
        <Button isIconOnly aria-label="Icon only" className={iconButton()}>
          *
        </Button>
      </div>,
    );

    expect(
      screen.getByRole("button", { name: "Save changes" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Disabled action" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Pending action" }),
    ).toHaveAttribute("data-pending", "true");
    expect(
      screen.getByRole("button", { name: "Delete item" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Icon only" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it("supports controlled TextField value and field error", async () => {
    const user = userEvent.setup();

    function ControlledField() {
      const [value, setValue] = useState("");
      const invalid = value.length > 0 && value.length < 3;
      return (
        <TextField
          isInvalid={invalid}
          name="username"
          value={value}
          onChange={setValue}
        >
          <Label>Username</Label>
          <Input placeholder="jane" />
          {invalid ? (
            <FieldError>Username must be at least 3 characters.</FieldError>
          ) : (
            <Description>Choose a unique username.</Description>
          )}
        </TextField>
      );
    }

    render(<ControlledField />);

    const input = screen.getByRole("textbox", { name: "Username" });
    expect(input).toHaveValue("");
    expect(screen.getByText("Choose a unique username.")).toBeInTheDocument();

    await user.type(input, "ab");
    expect(input).toHaveValue("ab");
    expect(
      screen.getByText("Username must be at least 3 characters."),
    ).toBeInTheDocument();

    await user.type(input, "c");
    expect(input).toHaveValue("abc");
    expect(screen.getByText("Choose a unique username.")).toBeInTheDocument();
  });

  it("supports controlled Switch selection", async () => {
    const user = userEvent.setup();

    function ControlledSwitch() {
      const [selected, setSelected] = useState(false);
      return (
        <Switch isSelected={selected} onChange={setSelected}>
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
            Enable feature
          </Switch.Content>
        </Switch>
      );
    }

    render(<ControlledSwitch />);
    const toggle = screen.getByRole("switch", { name: "Enable feature" });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(toggle).toBeChecked();
  });
});
