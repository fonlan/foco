#!/usr/bin/env node

/**
 * Reports native interactive JSX left in production code. This is deliberately
 * a coverage report, not a component detector: semantic CSS tokens never count
 * as a HeroUI component migration.
 *
 * Usage: npm run audit:heroui
 */
import { readdir, readFile } from "node:fs/promises";
import { basename, relative, resolve } from "node:path";
import ts from "typescript";

const workspaceRoot = basename(process.cwd()) === "web"
  ? resolve(process.cwd(), "..")
  : process.cwd();
const root = resolve(workspaceRoot, "web");
const productionDirectoryNames = new Set(["__tests__", "dist", "node_modules", "test-utils"]);

const targetFor = (kind, inputType) => {
  if (kind === "button") return { target: "Button", status: "migrate" };
  if (kind === "textarea") return { target: "TextArea", status: "migrate" };
  if (kind === "select") return { target: "Select + ListBox", status: "migrate" };
  if (kind === "[role=dialog]" || kind === "dialog") {
    return { target: "Modal", status: "migrate" };
  }

  // Browser file picking still needs the actual file-input element. Its visible
  // trigger must be HeroUI Button, and the exception ends once HeroUI exposes a
  // browser-file capability that keeps the same user-agent permission flow.
  if (inputType === "file") {
    return {
      target: "browser file input (written exception)",
      status: "exception",
      reason: "Browser file selection requires an actual input[type=file].",
      owner: "The visible trigger remains a HeroUI Button; the input stays labelled and hidden.",
      removal: "Remove when HeroUI offers an equivalent browser-file capability.",
    };
  }

  if (inputType === "checkbox") {
    return { target: "Checkbox or Switch", status: "migrate" };
  }

  if (inputType === "radio") {
    return { target: "RadioGroup + Radio", status: "migrate" };
  }

  return { target: "TextField + Input", status: "migrate" };
};

async function filesIn(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) {
        return productionDirectoryNames.has(entry.name) ? [] : filesIn(path);
      }
      return entry.isFile() && /\.tsx$/.test(entry.name) && !/\.(test|spec)\.tsx$/.test(entry.name)
        ? [path]
        : [];
    }),
  );
  return nested.flat();
}

const rows = [];
for (const file of await filesIn(root)) {
  const source = await readFile(file, "utf8");
  const sourceFile = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const attributeValue = (attributes, name) => {
    const attribute = attributes.properties.find(
      (property) => ts.isJsxAttribute(property) && property.name.text === name,
    );
    return attribute && attribute.initializer && ts.isStringLiteral(attribute.initializer)
      ? attribute.initializer.text
      : undefined;
  };
  const visit = (node) => {
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const element = node.tagName.getText(sourceFile);
      const inputType = element === "input" ? attributeValue(node.attributes, "type")?.toLowerCase() : undefined;
      const role = attributeValue(node.attributes, "role");
      const kind = ["button", "input", "select", "textarea", "dialog"].includes(element)
        ? element
        : role === "dialog"
          ? "[role=dialog]"
          : undefined;

      if (kind) {
        const line = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
        rows.push({
          file: relative(workspaceRoot, file),
          line,
          kind,
          ...(kind === "input" ? { inputType: inputType ?? "text" } : {}),
          ...targetFor(kind, inputType),
        });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
}

const byFile = new Map();
for (const row of rows) {
  const group = byFile.get(row.file) ?? [];
  group.push(row);
  byFile.set(row.file, group);
}

console.log("# HeroUI component coverage audit\n");
console.log(`Native JSX controls: ${rows.length}`);
console.log(`Mapped for migration: ${rows.filter((row) => row.status === "migrate").length}`);
console.log(`Written exceptions: ${rows.filter((row) => row.status === "exception").length}\n`);
console.log("This report maps native JSX only. CSS semantic-token usage is not component coverage.\n");

for (const [file, fileRows] of [...byFile].sort(([a], [b]) => a.localeCompare(b))) {
  const summary = Object.entries(
    Object.groupBy(fileRows, (row) => `${row.kind} → ${row.target}`),
  )
    .map(([label, entries]) => `${label}: ${entries.length}`)
    .join(", ");
  console.log(`## ${file}\n${summary}\n`);
  for (const row of fileRows) {
    const exception = row.status === "exception"
      ? ` — exception: ${row.reason} Accessibility: ${row.owner} Remove: ${row.removal}`
      : "";
    const source = row.kind === "[role=dialog]"
      ? "[role=\"dialog\"]"
      : `<${row.kind}${row.inputType ? ` type=\"${row.inputType}\"` : ""}>`;
    console.log(`- L${row.line}: ${source} → ${row.target}${exception}`);
  }
  console.log("");
}
