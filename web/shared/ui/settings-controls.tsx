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
    return <input {...props} ref={ref} disabled={disabled} onChange={onChange} type="file" value={value} />;
  }

  if (type === "checkbox") {
    return (
      <Switch
        {...(props as any)}
        aria-label={props["aria-label"]}
        isDisabled={disabled}
        isSelected={Boolean(checked)}
        onChange={(selected: boolean) => onChange?.(browserChange<HTMLInputElement>(selected ? "on" : "", selected))}
      >
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
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
  const change = (next: string) => onChange?.(browserChange<HTMLSelectElement>(next));

  return (
    <Select
      {...(props as any)}
      aria-label={props["aria-label"] ?? "Setting select"}
      className={className}
      isDisabled={disabled}
      selectionMode={multiple ? "multiple" : "single"}
      value={value ?? null}
      onChange={(next: string | number | null) => change(String(next ?? ""))}
    >
      <Select.Trigger>
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
