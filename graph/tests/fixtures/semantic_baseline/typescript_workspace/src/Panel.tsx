import { render } from "./index";

export function Panel({ value }: { value: string }) {
  return <section>{render(value)}</section>;
}
