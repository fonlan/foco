import { formatLabel as label } from "./public";

export function render(value: string): string {
  const result = label(value);
  return result;
}
