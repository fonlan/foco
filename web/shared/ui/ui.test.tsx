import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { DeleteChatDialog } from "../../features/chat/DeleteChatDialog";
import {
  Button,
  ContextMenu,
  Description,
  FieldError,
  Input,
  Label,
  Modal,
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

  it("Modal provides dialog role, Escape dismissal, and footer actions", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onConfirm = vi.fn();

    function ControlledModal() {
      const [open, setOpen] = useState(true);
      return (
        <Modal.Backdrop
          isDismissable
          isOpen={open}
          onOpenChange={(next) => {
            setOpen(next);
            if (!next) {
              onClose();
            }
          }}
        >
          <Modal.Container>
            <Modal.Dialog aria-label="Confirm delete">
              <Modal.Header>
                <Modal.Heading>Confirm delete</Modal.Heading>
              </Modal.Header>
              <Modal.Body>
                <p>Delete this item permanently?</p>
              </Modal.Body>
              <Modal.Footer>
                <Button variant="tertiary" onPress={() => setOpen(false)}>
                  Cancel
                </Button>
                <Button
                  variant="danger"
                  onPress={() => {
                    onConfirm();
                    setOpen(false);
                  }}
                >
                  Confirm
                </Button>
              </Modal.Footer>
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      );
    }

    render(<ControlledModal />);

    const dialog = screen.getByRole("dialog", { name: "Confirm delete" });
    expect(dialog).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "Cancel" }),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "Confirm" }),
    ).toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("ContextMenu exposes menu items and invokes onAction", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const onOpenChange = vi.fn();

    render(
      <ContextMenu
        aria-label="File actions"
        isOpen
        items={[
          { id: "open", label: "Open" },
          { danger: true, id: "delete", label: "Delete" },
        ]}
        left={40}
        top={80}
        onAction={onAction}
        onOpenChange={onOpenChange}
      />,
    );

    const menu = await screen.findByRole("menu");
    expect(menu).toHaveAttribute("aria-label", "File actions");
    expect(within(menu).getByRole("menuitem", { name: "Open" })).toBeInTheDocument();
    expect(
      within(menu).getByRole("menuitem", { name: "Delete" }),
    ).toBeInTheDocument();

    await user.click(within(menu).getByRole("menuitem", { name: "Open" }));
    expect(onAction).toHaveBeenCalled();
    expect(String(onAction.mock.calls[0]?.[0])).toBe("open");
  });
});

describe("DeleteChatDialog HeroUI Modal", () => {
  it("renders dialog name and confirm/cancel actions", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onConfirm = vi.fn();

    render(
      <DeleteChatDialog
        chat={{
          chatId: "chat-1",
          title: "Refactor plan",
          workspaceId: "ws-1",
          workspaceName: "Foco",
        }}
        onClose={onClose}
        onConfirm={onConfirm}
      />,
    );

    const dialog = screen.getByRole("dialog", { name: "Delete this chat?" });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("Refactor plan")).toBeInTheDocument();
    expect(screen.getByText("Foco")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Cancel chat deletion" }));
    expect(onClose).toHaveBeenCalled();

    onClose.mockClear();
    render(
      <DeleteChatDialog
        chat={{
          chatId: "chat-1",
          title: "Refactor plan",
          workspaceId: "ws-1",
          workspaceName: "Foco",
        }}
        onClose={onClose}
        onConfirm={onConfirm}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Confirm delete chat" }));
    expect(onConfirm).toHaveBeenCalled();
  });
});
