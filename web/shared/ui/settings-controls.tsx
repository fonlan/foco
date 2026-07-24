import {
  Button,
  Checkbox,
  Input,
  Label,
  ListBox,
  Radio,
  Select,
  Switch,
  TextArea,
  TextField,
} from "@heroui/react";
import {
  Children,
  forwardRef,
  isValidElement,
  type ButtonHTMLAttributes,
  type ChangeEvent,
  type ChangeEventHandler,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

const RadioControl: any = Radio;

/**
 * Compatibility bridge for settings forms while their data-owning feature
 * components move to HeroUI. It deliberately keeps the browser-shaped change
 * callbacks used by existing reducers; the rendered controls are HeroUI v3
 * compound components, not native fallbacks.
 *
 * New UI should compose HeroUI directly. These exports exist only to make the
 * large settings migration behavior-preserving and are kept in shared/ui so
 * the feature file does not accumulate another local pseudo component set.
 */
export function SettingsButton({ disabled, onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <Button
      {...(props as any)}
      isDisabled={disabled}
      onPress={onClick ? (event) => onClick({ ...event, stopPropagation: () => {} } as any) : undefined}
      variant="ghost"
    />
  );
}

function browserChange<T extends HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
  value: string,
  checked?: boolean,
) {
  return { target: { checked, value } } as unknown as ChangeEvent<T>;
}

type SettingsInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "onChange"> & {
  onChange?: ChangeEventHandler<HTMLInputElement>;
};

export const SettingsInput = forwardRef<HTMLInputElement, SettingsInputProps>(function SettingsInput({
  checked,
  children: _children,
  disabled,
  onChange,
  type = "text",
  value,
  ...props
}: SettingsInputProps, ref) {
  if (type === "file") {
    // Browser file selection remains the documented native-control exception.
    return <input {...props} data-heroui-exception="native-file-input" ref={ref} disabled={disabled} onChange={onChange} type="file" value={value} />;
  }

  if (type === "checkbox") {
    if (props.role === "switch") {
      return (
        <Switch
          {...(props as any)}
          aria-label={props["aria-label"]}
          isDisabled={disabled}
          isSelected={Boolean(checked)}
          onChange={(selected: boolean) =>
            onChange?.(browserChange<HTMLInputElement>(selected ? "on" : "", selected))
          }
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
      );
    }

    return (
      <Checkbox
        {...(props as any)}
        aria-label={props["aria-label"]}
        isDisabled={disabled}
        isSelected={Boolean(checked)}
        onChange={(selected: boolean) => onChange?.(browserChange<HTMLInputElement>(selected ? "on" : "", selected))}
      >
        <Checkbox.Content>
          <Checkbox.Control>
            <Checkbox.Indicator />
          </Checkbox.Control>
        </Checkbox.Content>
      </Checkbox>
    );
  }

  if (type === "radio") {
    return (
      <RadioControl
        {...(props as any)}
        isDisabled={disabled}
        isSelected={Boolean(checked)}
        onChange={(selected: boolean) => onChange?.(browserChange<HTMLInputElement>(String(value ?? ""), selected))}
        value={String(value ?? "")}
      >
        <Radio.Content>
          <Radio.Control>
            <Radio.Indicator />
          </Radio.Control>
        </Radio.Content>
      </RadioControl>
    );
  }

  return <Input {...(props as any)} ref={ref} disabled={disabled} onChange={onChange} type={type} value={value} />;
});

type SettingsTextAreaProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "onChange"> & {
  onChange?: ChangeEventHandler<HTMLTextAreaElement>;
};

export function SettingsTextArea({
  children: _children,
  disabled,
  onChange,
  value,
  ...props
}: SettingsTextAreaProps) {
  return <TextArea {...(props as any)} disabled={disabled} onChange={onChange} value={value} />;
}

function optionItems(children: ReactNode): Array<{ disabled?: boolean; id: string; text: string }> {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement(child)) {
      return [];
    }

    if (child.type === "optgroup") {
      return optionItems((child.props as { children?: ReactNode }).children);
    }

    const props = child.props as { children?: ReactNode; disabled?: boolean; value?: string | number };
    const text = Children.toArray(props.children).join("");
    return [{ disabled: props.disabled, id: String(props.value ?? text), text }];
  });
}

type SettingsSelectProps = Omit<SelectHTMLAttributes<HTMLSelectElement>, "onChange"> & {
  onChange?: ChangeEventHandler<HTMLSelectElement>;
};

export function SettingsSelect({
  children,
  className,
  disabled,
  multiple,
  onChange,
  value,
  ...props
}: SettingsSelectProps) {
  const items = optionItems(children);
  const selectRef = useRef<HTMLElement>(null);
  const [inferredLabel, setInferredLabel] = useState<string | undefined>();
  const change = (next: string) => onChange?.(browserChange<HTMLSelectElement>(next));

  // Existing settings fields wrap their control with a visible HTML label. A
  // React Aria combobox is not a labelable HTML element, so that implicit
  // browser association does not carry over. Preserve the visible label as the
  // combobox's accessible name until call sites can be expressed as direct
  // HeroUI <Label> children.
  useLayoutEffect(() => {
    if (props["aria-label"] || props["aria-labelledby"]) return;

    const wrapper = selectRef.current?.closest("label");
    const label = wrapper?.querySelector("span")?.textContent?.trim();
    if (label && label !== inferredLabel) setInferredLabel(label);
  }, [inferredLabel, props]);

  return (
    <Select
      {...(props as any)}
      ref={selectRef as any}
      aria-label={props["aria-label"] ?? inferredLabel ?? "Setting select"}
      className="w-full"
      isDisabled={disabled}
      selectionMode={multiple ? "multiple" : "single"}
      selectedKey={multiple || value == null ? null : String(value)}
      selectedKeys={
        multiple
          ? new Set(Array.isArray(value) ? value.map(String) : value == null ? [] : [String(value)])
          : undefined
      }
      onSelectionChange={(next) => {
        const selection = next as unknown;
        const selected = selection instanceof Set ? [...(selection as Set<string>)][0] : next;
        change(selected == null ? "" : String(selected));
      }}
    >
      <Select.Trigger className={`${className ?? ""} shadow-none`}>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {items.map((item) => (
            <ListBox.Item key={item.id} id={item.id} isDisabled={item.disabled} textValue={item.text}>
              {item.text}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}

type SettingsTextFieldProps = {
  autoComplete?: string;
  disabled?: boolean;
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  type?: "password" | "text";
  value: string;
};

export function SettingsTextField({
  autoComplete = "off",
  disabled = false,
  inputMode,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
}: SettingsTextFieldProps) {
  return (
    <TextField
      autoComplete={autoComplete}
      className="block"
      isDisabled={disabled}
      name={String(label).toLowerCase().replace(/\s+/g, "-")}
      type={type}
      value={value}
      onChange={onChange}
    >
      <Label className="mb-1.5 block text-xs font-semibold text-muted">{label}</Label>
      <Input inputMode={inputMode} placeholder={placeholder} />
    </TextField>
  );
}
